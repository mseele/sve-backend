use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use iban::IbanLike;
use num_traits::ToPrimitive;
use quick_xml::Writer;
use quick_xml::events::Event as XmlEvent;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText};
use tracing::warn;
use uuid::Uuid;

use crate::error::ValidationError;
use crate::models::{Event, EventSubscription};

pub(crate) fn validate_iban(raw: &str) -> Result<String, ValidationError> {
    let iban: iban::Iban = raw.parse().map_err(|e| {
        warn!("IBAN ({}) validation failed for input: {}", raw, e);
        ValidationError::new("Bitte gib eine gültige IBAN ein.")
    })?;

    Ok(iban.electronic_str().to_string())
}

pub(crate) async fn lookup_bic(iban: &str) -> Result<String> {
    let parsed = iban
        .parse::<iban::Iban>()
        .map_err(|_| anyhow!("Invalid IBAN: {}", iban))?;

    if let Some(bank_code) = parsed.bank_identifier()
        && let Some(bank) = fints_institute_db::get_bank_by_bank_code(bank_code)
    {
        return Ok(bank.bic.to_string());
    }

    let url = format!("https://bankcheck.dev/api/v1/validate?q={}", iban);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("BIC lookup failed: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse BIC response: {}", e))?;

    if let Some(bic) = json
        .get("result")
        .and_then(|r| r.get("bankInfo"))
        .and_then(|bi| bi.get("bic"))
        .and_then(|b| b.as_str())
        && !bic.is_empty()
    {
        return Ok(bic.to_string());
    }

    bail!("No BIC found for IBAN: {}", iban)
}

fn write_element(writer: &mut Writer<Vec<u8>>, name: &str, value: &str) -> Result<()> {
    writer.write_event(XmlEvent::Start(BytesStart::new(name)))?;
    writer.write_event(XmlEvent::Text(BytesText::new(value)))?;
    writer.write_event(XmlEvent::End(BytesEnd::new(name)))?;
    Ok(())
}

fn write_element_with_attr(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    attr: (&str, &str),
    text: &str,
) -> Result<()> {
    let mut elem = BytesStart::new(name);
    elem.push_attribute(attr);
    writer.write_event(XmlEvent::Start(elem))?;
    writer.write_event(XmlEvent::Text(BytesText::new(text)))?;
    writer.write_event(XmlEvent::End(BytesEnd::new(name)))?;
    Ok(())
}

pub(crate) fn generate_sepa_xml(
    event: &Event,
    bookings: &[(EventSubscription, String)],
    creditor_name: &str,
    creditor_iban: &str,
    creditor_bic: &str,
) -> Result<String> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

    writer.write_event(XmlEvent::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let doc_attrs = vec![
        ("xmlns", "urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"),
        ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    ];
    let mut doc = BytesStart::new("Document");
    for (k, v) in &doc_attrs {
        doc.push_attribute((*k, *v));
    }
    writer.write_event(XmlEvent::Start(doc.clone()))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("CstmrDrctDbtInitn")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("GrpHdr")))?;
    let msg_id = Uuid::new_v4().to_string();
    write_element(&mut writer, "MsgId", &msg_id)?;
    write_element(
        &mut writer,
        "CreDtTm",
        &Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
    )?;
    write_element(&mut writer, "NbOfTxs", &bookings.len().to_string())?;

    let ctrl_sum: f64 = bookings
        .iter()
        .map(|(sub, _)| event.price(sub.member).to_f64().unwrap_or(0.0))
        .sum();
    write_element(&mut writer, "CtrlSum", &format!("{:.2}", ctrl_sum))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("InitgPty")))?;
    write_element(&mut writer, "Nm", creditor_name)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("InitgPty")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("GrpHdr")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("PmtInf")))?;
    let pmt_inf_id = Uuid::new_v4().to_string();
    write_element(&mut writer, "PmtInfId", &pmt_inf_id)?;
    write_element(&mut writer, "PmtMtd", "DD")?;
    write_element(&mut writer, "NbOfTxs", &bookings.len().to_string())?;
    write_element(&mut writer, "CtrlSum", &format!("{:.2}", ctrl_sum))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("PmtTpInf")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("SvcLvl")))?;
    write_element(&mut writer, "Cd", "SEPA")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("SvcLvl")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("LclInstrm")))?;
    write_element(&mut writer, "Cd", "CORE")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("LclInstrm")))?;
    write_element(&mut writer, "SeqTp", "FRST")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("PmtTpInf")))?;

    write_element(
        &mut writer,
        "ReqdColltnDt",
        &Utc::now().format("%Y-%m-%d").to_string(),
    )?;

    writer.write_event(XmlEvent::Start(BytesStart::new("Cdtr")))?;
    write_element(&mut writer, "Nm", creditor_name)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("Cdtr")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("CdtrAcct")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("Id")))?;
    write_element(&mut writer, "IBAN", creditor_iban)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("Id")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("CdtrAcct")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("CdtrAgt")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("FinInstnId")))?;
    write_element(&mut writer, "BIC", creditor_bic)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("FinInstnId")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("CdtrAgt")))?;

    write_element(&mut writer, "ChrgBr", "SLEV")?;

    for (sub, bic) in bookings {
        let price = event.price(sub.member);
        let mandate_ref = format!("SEPA-{}", sub.payment_id);
        let sign_date = sub.created.format("%Y-%m-%d").to_string();

        writer.write_event(XmlEvent::Start(BytesStart::new("DrctDbtTxInf")))?;

        writer.write_event(XmlEvent::Start(BytesStart::new("PmtId")))?;
        write_element(&mut writer, "EndToEndId", &mandate_ref)?;
        writer.write_event(XmlEvent::End(BytesEnd::new("PmtId")))?;

        write_element_with_attr(
            &mut writer,
            "InstdAmt",
            ("Ccy", "EUR"),
            &format!("{:.2}", price),
        )?;

        writer.write_event(XmlEvent::Start(BytesStart::new("DrctDbtTx")))?;
        writer.write_event(XmlEvent::Start(BytesStart::new("MndtRltdInf")))?;
        write_element(&mut writer, "MndtId", &mandate_ref)?;
        write_element(&mut writer, "DtOfSgntr", &sign_date)?;
        writer.write_event(XmlEvent::End(BytesEnd::new("MndtRltdInf")))?;
        writer.write_event(XmlEvent::End(BytesEnd::new("DrctDbtTx")))?;

        writer.write_event(XmlEvent::Start(BytesStart::new("DbtrAgt")))?;
        writer.write_event(XmlEvent::Start(BytesStart::new("FinInstnId")))?;
        write_element(&mut writer, "BIC", bic)?;
        writer.write_event(XmlEvent::End(BytesEnd::new("FinInstnId")))?;
        writer.write_event(XmlEvent::End(BytesEnd::new("DbtrAgt")))?;

        writer.write_event(XmlEvent::Start(BytesStart::new("Dbtr")))?;
        write_element(
            &mut writer,
            "Nm",
            &format!("{} {}", sub.first_name, sub.last_name),
        )?;
        writer.write_event(XmlEvent::End(BytesEnd::new("Dbtr")))?;

        let iban = sub.iban.as_deref().unwrap_or("");
        writer.write_event(XmlEvent::Start(BytesStart::new("DbtrAcct")))?;
        writer.write_event(XmlEvent::Start(BytesStart::new("Id")))?;
        write_element(&mut writer, "IBAN", iban)?;
        writer.write_event(XmlEvent::End(BytesEnd::new("Id")))?;
        writer.write_event(XmlEvent::End(BytesEnd::new("DbtrAcct")))?;

        writer.write_event(XmlEvent::Start(BytesStart::new("RmtInf")))?;
        write_element(
            &mut writer,
            "Ustrd",
            &format!("Teilnahmegebühr {}", event.name),
        )?;
        writer.write_event(XmlEvent::End(BytesEnd::new("RmtInf")))?;

        writer.write_event(XmlEvent::End(BytesEnd::new("DrctDbtTxInf")))?;
    }

    writer.write_event(XmlEvent::End(BytesEnd::new("PmtInf")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("CstmrDrctDbtInitn")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("Document")))?;

    Ok(String::from_utf8(writer.into_inner())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Event, EventSubscription, EventType, LifecycleStatus, PaymentMethod};
    use bigdecimal::BigDecimal;
    use chrono::Utc;

    #[test]
    fn test_validate_iban() {
        let result = validate_iban("DE89 3704 0044 0532 0130 00").unwrap();
        assert_eq!(result, "DE89370400440532013000");

        let result = validate_iban("DE89370400440532013000").unwrap();
        assert_eq!(result, "DE89370400440532013000");

        let result = validate_iban("FR1420041010050500013M02606").unwrap();
        assert_eq!(result, "FR1420041010050500013M02606");

        let result = validate_iban("DE00000000000000000000");
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_sepa_xml_structure() {
        let event = Event::new(
            1,
            Utc::now(),
            None,
            EventType::Events,
            LifecycleStatus::Published,
            "Test Event".to_string(),
            0,
            "Short".to_string(),
            "Desc".to_string(),
            "img.png".to_string(),
            false,
            vec![],
            None,
            60,
            10,
            5,
            BigDecimal::from(20),
            BigDecimal::from(25),
            None,
            "Location".to_string(),
            "Template".to_string(),
            None,
            None,
            None,
            false,
            vec![],
            PaymentMethod::SepaDirectDebit,
        );

        let subscriber = EventSubscription::new(
            1,
            Utc::now(),
            "Max".to_string(),
            "Mustermann".to_string(),
            "Teststr 1".to_string(),
            "Teststadt".to_string(),
            "max@test.com".to_string(),
            None,
            true,
            true,
            "PAY123".to_string(),
            None,
            None,
            Some("DE89370400440532013000".to_string()),
            None,
            vec![],
        );

        let xml = generate_sepa_xml(
            &event,
            &[(subscriber, "COBADEFFXXX".to_string())],
            "Test Creditor",
            "DE89370400440532013000",
            "COBADEFFXXX",
        )
        .unwrap();

        assert!(xml.contains(r#"xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02""#));
        assert!(xml.contains("<CstmrDrctDbtInitn>"));
        assert!(xml.contains("<GrpHdr>"));
        assert!(xml.contains("<PmtInf>"));
        assert!(xml.contains("<PmtMtd>DD</PmtMtd>"));
        assert!(xml.contains("<Cd>SEPA</Cd>"));
        assert!(xml.contains("<Cd>CORE</Cd>"));
        assert!(xml.contains("<SeqTp>FRST</SeqTp>"));
        assert!(xml.contains("<Nm>Test Creditor</Nm>"));
        assert!(xml.contains("<IBAN>DE89370400440532013000</IBAN>"));
        assert!(xml.contains("<BIC>COBADEFFXXX</BIC>"));
        assert!(xml.contains("<EndToEndId>SEPA-PAY123</EndToEndId>"));
        assert!(xml.contains("<InstdAmt Ccy=\"EUR\">20.00</InstdAmt>"));
        assert!(xml.contains("<Nm>Max Mustermann</Nm>"));
        assert!(xml.contains("<ChrgBr>SLEV</ChrgBr>"));
        assert!(xml.contains("<CtrlSum>20.00</CtrlSum>"));
        assert!(xml.contains("<NbOfTxs>1</NbOfTxs>"));
        assert!(xml.contains("<MndtId>SEPA-PAY123</MndtId>"));
        assert!(xml.contains("<Ustrd>Teilnahmegebühr Test Event</Ustrd>"));
    }
}
