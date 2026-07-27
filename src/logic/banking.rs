use anyhow::{Result, anyhow, bail};
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use iban::IbanLike;
use num_traits::ToPrimitive;
use quick_xml::Writer;
use quick_xml::events::Event as XmlEvent;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText};
use tracing::warn;
use uuid::Uuid;

use crate::error::ValidationError;
use crate::models::{Event, EventSubscription};

pub(crate) fn validate_iban(raw: &str) -> Result<iban::Iban, ValidationError> {
    let normalized: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    normalized.parse().map_err(|_| {
        warn!("IBAN validation failed for input: {}", normalized);
        ValidationError::new("Bitte gib eine gültige IBAN ein.")
    })
}

pub(crate) fn validate_iban_str(raw: &str) -> Result<String, ValidationError> {
    Ok(validate_iban(raw)?.electronic_str().to_string())
}

fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let base = h + l - 7 * m + 114;
    let month = (base / 31) as u32;
    let day = ((base % 31) + 1) as u32;
    NaiveDate::from_ymd_opt(year, month, day).expect("valid easter date")
}

fn is_sepa_banking_day(date: NaiveDate) -> bool {
    if matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
        return false;
    }
    let (m, d) = (date.month(), date.day());
    if matches!((m, d), (1, 1) | (5, 1) | (12, 25) | (12, 26)) {
        return false;
    }
    let easter = easter_sunday(date.year());
    if date == easter - Duration::days(2) || date == easter + Duration::days(1) {
        return false;
    }
    true
}

fn add_sepa_banking_days(from: NaiveDate, count: u32) -> NaiveDate {
    let mut date = from;
    let mut remaining = count;
    while remaining > 0 {
        date += Duration::days(1);
        if is_sepa_banking_day(date) {
            remaining -= 1;
        }
    }
    date
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum SepaSequenceType {
    First,
    OneOff,
    Recurring,
}

impl AsRef<str> for SepaSequenceType {
    fn as_ref(&self) -> &str {
        match self {
            SepaSequenceType::First => "FRST",
            SepaSequenceType::OneOff => "OOFF",
            SepaSequenceType::Recurring => "RCUR",
        }
    }
}

fn sepa_lead_days(seq_tp: SepaSequenceType) -> u32 {
    match seq_tp {
        SepaSequenceType::First | SepaSequenceType::OneOff => 5,
        SepaSequenceType::Recurring => 2,
    }
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
    creditor_id: &str,
) -> Result<String> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

    writer.write_event(XmlEvent::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let doc_attrs = vec![
        ("xmlns", "urn:iso:std:iso:20022:tech:xsd:pain.008.001.08"),
        ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
        (
            "xsi:schemaLocation",
            "urn:iso:std:iso:20022:tech:xsd:pain.008.001.08 pain.008.001.08.xsd",
        ),
    ];
    let mut doc = BytesStart::new("Document");
    for (k, v) in &doc_attrs {
        doc.push_attribute((*k, *v));
    }
    writer.write_event(XmlEvent::Start(doc.clone()))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("CstmrDrctDbtInitn")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("GrpHdr")))?;
    let msg_id = Uuid::new_v4().simple().to_string();
    write_element(&mut writer, "MsgId", &msg_id)?;
    write_element(
        &mut writer,
        "CreDtTm",
        &Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    )?;
    write_element(&mut writer, "NbOfTxs", &bookings.len().to_string())?;

    let ctrl_sum: f64 = bookings
        .iter()
        .map(|(sub, _)| sub.total_price(event).to_f64().unwrap_or(0.0))
        .sum();
    write_element(&mut writer, "CtrlSum", &format!("{:.2}", ctrl_sum))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("InitgPty")))?;
    write_element(&mut writer, "Nm", creditor_name)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("InitgPty")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("GrpHdr")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("PmtInf")))?;
    let pmt_inf_id = Uuid::new_v4().simple().to_string();
    write_element(&mut writer, "PmtInfId", &pmt_inf_id)?;
    write_element(&mut writer, "PmtMtd", "DD")?;
    write_element(&mut writer, "BtchBookg", "true")?;
    write_element(&mut writer, "NbOfTxs", &bookings.len().to_string())?;
    write_element(&mut writer, "CtrlSum", &format!("{:.2}", ctrl_sum))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("PmtTpInf")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("SvcLvl")))?;
    write_element(&mut writer, "Cd", "SEPA")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("SvcLvl")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("LclInstrm")))?;
    write_element(&mut writer, "Cd", "CORE")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("LclInstrm")))?;
    let seq_tp = SepaSequenceType::OneOff;
    write_element(&mut writer, "SeqTp", seq_tp.as_ref())?;
    writer.write_event(XmlEvent::Start(BytesStart::new("CtgyPurp")))?;
    write_element(&mut writer, "Cd", "OTHR")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("CtgyPurp")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("PmtTpInf")))?;

    let coll_dt = add_sepa_banking_days(Utc::now().date_naive(), sepa_lead_days(seq_tp));
    write_element(
        &mut writer,
        "ReqdColltnDt",
        &coll_dt.format("%Y-%m-%d").to_string(),
    )?;

    writer.write_event(XmlEvent::Start(BytesStart::new("Cdtr")))?;
    write_element(&mut writer, "Nm", creditor_name)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("Cdtr")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("CdtrAcct")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("Id")))?;
    write_element(&mut writer, "IBAN", creditor_iban)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("Id")))?;
    write_element(&mut writer, "Ccy", "EUR")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("CdtrAcct")))?;

    writer.write_event(XmlEvent::Start(BytesStart::new("CdtrAgt")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("FinInstnId")))?;
    write_element(&mut writer, "BICFI", creditor_bic)?;
    writer.write_event(XmlEvent::End(BytesEnd::new("FinInstnId")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("CdtrAgt")))?;

    write_element(&mut writer, "ChrgBr", "SLEV")?;

    writer.write_event(XmlEvent::Start(BytesStart::new("CdtrSchmeId")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("Id")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("PrvtId")))?;
    writer.write_event(XmlEvent::Start(BytesStart::new("Othr")))?;
    write_element(&mut writer, "Id", creditor_id)?;
    writer.write_event(XmlEvent::Start(BytesStart::new("SchmeNm")))?;
    write_element(&mut writer, "Prtry", "SEPA")?;
    writer.write_event(XmlEvent::End(BytesEnd::new("SchmeNm")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("Othr")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("PrvtId")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("Id")))?;
    writer.write_event(XmlEvent::End(BytesEnd::new("CdtrSchmeId")))?;

    for (sub, bic) in bookings {
        let price = sub.total_price(event);
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
        write_element(&mut writer, "AmdmntInd", "false")?;
        writer.write_event(XmlEvent::End(BytesEnd::new("MndtRltdInf")))?;
        writer.write_event(XmlEvent::End(BytesEnd::new("DrctDbtTx")))?;

        writer.write_event(XmlEvent::Start(BytesStart::new("DbtrAgt")))?;
        writer.write_event(XmlEvent::Start(BytesStart::new("FinInstnId")))?;
        write_element(&mut writer, "BICFI", bic)?;
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
    use bigdecimal::BigDecimal;
    use chrono::Utc;

    use crate::models::{
        Event, EventCustomField, EventCustomFieldType, EventSubscription, EventType,
        LifecycleStatus, PaymentMethod,
    };

    use super::*;

    #[test]
    fn test_validate_iban_str() {
        let result = validate_iban_str("DE89 3704 0044 0532 0130 00").unwrap();
        assert_eq!(result, "DE89370400440532013000");

        let result = validate_iban_str("DE89370400440532013000").unwrap();
        assert_eq!(result, "DE89370400440532013000");

        let result = validate_iban_str("FR1420041010050500013M02606").unwrap();
        assert_eq!(result, "FR1420041010050500013M02606");

        let result = validate_iban_str("de89370400440532013000").unwrap();
        assert_eq!(result, "DE89370400440532013000");

        let result = validate_iban_str("DE00000000000000000000");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_iban_str_strips_arbitrary_whitespace() {
        // Regression: production inputs with trailing whitespace or non-ISO
        // groupings (e.g. groups of 3, leading "DE ") must be accepted as long
        // as the stripped IBAN is valid. The `iban` crate only tolerates the
        // canonical 4-char paper grouping, so we must strip all whitespace.
        let cases = [
            ("DE98642910100036522007 ", "DE98642910100036522007"),
            ("DE32 642 910 100 034 586 008", "DE32642910100034586008"),
            ("DE 65 6425 1060 0000 7558 81", "DE65642510600000755881"),
            ("DE 65 6425 1060 0000 755881", "DE65642510600000755881"),
            ("  de98\t6429\n1010\r0036 522007 ", "DE98642910100036522007"),
        ];
        for (input, expected) in cases {
            let result = validate_iban_str(input)
                .unwrap_or_else(|e| panic!("expected valid IBAN for {input:?}: {e:?}"));
            assert_eq!(result, expected, "wrong normalization for {input:?}");
        }
    }

    #[test]
    fn test_easter_sunday_known_values() {
        assert_eq!(
            easter_sunday(2024),
            NaiveDate::from_ymd_opt(2024, 3, 31).unwrap()
        );
        assert_eq!(
            easter_sunday(2025),
            NaiveDate::from_ymd_opt(2025, 4, 20).unwrap()
        );
        assert_eq!(
            easter_sunday(2026),
            NaiveDate::from_ymd_opt(2026, 4, 5).unwrap()
        );
        assert_eq!(
            easter_sunday(2027),
            NaiveDate::from_ymd_opt(2027, 3, 28).unwrap()
        );
    }

    #[test]
    fn test_is_sepa_banking_day() {
        let is_banking_day =
            |y, m, d| is_sepa_banking_day(NaiveDate::from_ymd_opt(y, m, d).unwrap());
        assert!(is_banking_day(2026, 7, 31), "Fri is a banking day");
        assert!(!is_banking_day(2026, 7, 25), "Sat is not");
        assert!(!is_banking_day(2026, 7, 26), "Sun is not");
        assert!(!is_banking_day(2026, 1, 1), "New Year");
        assert!(!is_banking_day(2026, 5, 1), "Labour Day");
        assert!(!is_banking_day(2026, 12, 25), "Christmas Day");
        assert!(!is_banking_day(2026, 12, 26), "Boxing Day");
        assert!(!is_banking_day(2026, 4, 3), "Good Friday 2026");
        assert!(!is_banking_day(2026, 4, 6), "Easter Monday 2026");
        assert!(
            is_banking_day(2026, 4, 7),
            "Tue after Easter Monday is a banking day"
        );
    }

    #[test]
    fn test_add_sepa_banking_days_skips_weekend() {
        let fri = NaiveDate::from_ymd_opt(2026, 7, 31).unwrap();
        let plus2 = add_sepa_banking_days(fri, 2);
        assert_eq!(plus2, NaiveDate::from_ymd_opt(2026, 8, 4).unwrap());

        let thursday_before_easter = NaiveDate::from_ymd_opt(2026, 4, 2).unwrap();
        let plus1 = add_sepa_banking_days(thursday_before_easter, 1);
        assert_eq!(plus1, NaiveDate::from_ymd_opt(2026, 4, 7).unwrap());

        let plus5 = add_sepa_banking_days(thursday_before_easter, 5);
        assert_eq!(plus5, NaiveDate::from_ymd_opt(2026, 4, 13).unwrap());
    }

    #[test]
    fn test_sepa_lead_days_matches_epc_core_rulebook() {
        assert_eq!(sepa_lead_days(SepaSequenceType::First), 5);
        assert_eq!(sepa_lead_days(SepaSequenceType::OneOff), 5);
        assert_eq!(sepa_lead_days(SepaSequenceType::Recurring), 2);
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
            "DE98ZZZ00000000001",
        )
        .unwrap();

        assert!(xml.contains(r#"xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.08""#));
        assert!(xml.contains("xsi:schemaLocation"));
        assert!(xml.contains("<CstmrDrctDbtInitn>"));
        assert!(xml.contains("<GrpHdr>"));
        assert!(xml.contains("<PmtInf>"));
        assert!(xml.contains("<PmtMtd>DD</PmtMtd>"));
        assert!(xml.contains("<BtchBookg>true</BtchBookg>"));
        assert!(xml.contains("<Cd>SEPA</Cd>"));
        assert!(xml.contains("<Cd>CORE</Cd>"));
        assert!(xml.contains("<SeqTp>OOFF</SeqTp>"));
        assert!(xml.contains("<CtgyPurp>"));
        assert!(xml.contains("<Cd>OTHR</Cd>"));
        assert!(xml.contains("<Nm>Test Creditor</Nm>"));
        assert!(xml.contains("<IBAN>DE89370400440532013000</IBAN>"));
        assert!(xml.contains("<Ccy>EUR</Ccy>"));
        assert!(xml.contains("<BICFI>COBADEFFXXX</BICFI>"));
        assert!(xml.contains("<EndToEndId>SEPA-PAY123</EndToEndId>"));
        assert!(xml.contains("<InstdAmt Ccy=\"EUR\">20.00</InstdAmt>"));
        assert!(xml.contains("<Nm>Max Mustermann</Nm>"));
        assert!(xml.contains("<ChrgBr>SLEV</ChrgBr>"));
        assert!(xml.contains("<CtrlSum>20.00</CtrlSum>"));
        assert!(xml.contains("<NbOfTxs>1</NbOfTxs>"));
        assert!(xml.contains("<MndtId>SEPA-PAY123</MndtId>"));
        assert!(xml.contains("<AmdmntInd>false</AmdmntInd>"));
        assert!(xml.contains("<Ustrd>Teilnahmegebühr Test Event</Ustrd>"));
        assert!(xml.contains("<CdtrSchmeId>"));
        assert!(xml.contains("<Id>DE98ZZZ00000000001</Id>"));
        assert!(xml.contains("<Prtry>SEPA</Prtry>"));
        assert!(
            xml.contains("Z</CreDtTm>"),
            "CreDtTm should use a Z-suffix UTC timestamp"
        );

        let re = regex::Regex::new(r"<ReqdColltnDt>(\d{4}-\d{2}-\d{2})</ReqdColltnDt>").unwrap();
        let coll_dt_str = re
            .captures(&xml)
            .expect("ReqdColltnDt element present")
            .get(1)
            .unwrap()
            .as_str();
        let coll_dt = NaiveDate::parse_from_str(coll_dt_str, "%Y-%m-%d")
            .expect("ReqdColltnDt is a valid ISO date");
        let today = Utc::now().date_naive();
        assert!(
            is_sepa_banking_day(coll_dt),
            "ReqdColltnDt must be a SEPA banking day, got {coll_dt}"
        );
        assert!(
            coll_dt >= add_sepa_banking_days(today, sepa_lead_days(SepaSequenceType::Recurring)),
            "ReqdColltnDt must be today + SEPA CORE RCUR lead time, got {coll_dt}"
        );

        // Validate against the canonical pain.008.001.08 XSD
        // (bundled at src/assets/pain.008.001.08.xsd) using the uppsala crate.
        let schema_xml = include_str!("../assets/pain.008.001.08.xsd");
        let schema = uppsala::parse(schema_xml).expect("parse XSD");
        let doc = uppsala::parse(&xml).expect("parse generated XML");
        let validator =
            uppsala::xsd::XsdValidator::from_schema(&schema).expect("compile XSD schema");
        let errors = validator.validate(&doc);
        assert!(
            errors.is_empty(),
            "SEPA XML failed pain.008.001.08 XSD validation: {errors:?}"
        );
    }

    #[test]
    fn test_generate_sepa_xml_with_price_relevant_field() {
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
            vec![EventCustomField::new(
                1,
                "Anzahl".to_string(),
                EventCustomFieldType::Number,
                None,
                None,
                true,
            )],
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
            vec![String::from("3")],
        );

        let xml = generate_sepa_xml(
            &event,
            &[(subscriber, "COBADEFFXXX".to_string())],
            "Test Creditor",
            "DE89370400440532013000",
            "COBADEFFXXX",
            "DE54ZZZ00000299406",
        )
        .unwrap();

        assert!(xml.contains(r#"<InstdAmt Ccy="EUR">60.00</InstdAmt>"#));
        assert!(xml.contains("<CtrlSum>60.00</CtrlSum>"));
    }
}
