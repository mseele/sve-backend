use crate::models::{FromEuro, MembershipApplication};
use anyhow::{Result, anyhow, bail};
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::Neg};
use tracing::warn;

pub(crate) fn read_payment_records(csv: &str) -> Result<Vec<PaymentRecord>> {
    read_voba_classic_csv(csv)
        .or_else(|e| {
            warn!("Failed to read as voba classic csv: {}", e);
            read_voba_rich_csv(csv)
        })
        .or_else(|e| {
            warn!("Failed to read as voba rich csv: {}", e);
            bail!("Unknown CSV format")
        })
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaymentRecord {
    pub(crate) date: NaiveDate,
    pub(crate) payee: String,
    pub(crate) payee_iban: String,
    pub(crate) purpose: String,
    pub(crate) volumne: BigDecimal,
    pub(crate) payment_ids: HashSet<String>,
}

impl PaymentRecord {
    fn new(
        date: NaiveDate,
        payee: String,
        payee_iban: String,
        purpose: String,
        volumne: BigDecimal,
    ) -> Self {
        // extract the payment id's
        lazy_static! {
            static ref PAYMENT_ID_PATTERN: Regex = Regex::new(r"\d{2}-\d{4}").unwrap();
        }
        let payment_ids = PAYMENT_ID_PATTERN
            .find_iter(&purpose)
            .map(|mat| mat.as_str().into())
            .collect();

        Self {
            date,
            payee,
            payee_iban,
            purpose,
            volumne,
            payment_ids,
        }
    }
}

// impl for voba csv file with header and footer

#[derive(Deserialize)]
struct VobaRichPaymentRecord {
    #[serde(
        rename = "Buchungstag",
        deserialize_with = "deserialize_date_with_german_format"
    )]
    date: NaiveDate,
    #[serde(rename = "Zahlungsempfänger")]
    payee: String,
    #[serde(rename = "ZahlungsempfängerIBAN")]
    payee_iban: String,
    #[serde(rename = "Vorgang/Verwendungszweck")]
    purpose: String,
    #[serde(rename = "Umsatz", deserialize_with = "deserialize_float_with_comma")]
    volumne: BigDecimal,
    #[serde(rename = "Soll/Haben")]
    debit_credit: String,
}

impl From<VobaRichPaymentRecord> for PaymentRecord {
    fn from(record: VobaRichPaymentRecord) -> Self {
        let volumne = record.volumne();
        PaymentRecord::new(
            record.date,
            record.payee,
            record.payee_iban,
            record.purpose,
            volumne,
        )
    }
}

impl VobaRichPaymentRecord {
    fn volumne(&self) -> BigDecimal {
        match self.debit_credit.as_str() {
            "H" => self.volumne.clone(),
            _ => self.volumne.clone().neg(),
        }
    }

    fn find_start(csv: &str) -> Option<usize> {
        let csv_records_prefix = "Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben";
        csv.find(csv_records_prefix)
    }
}

fn read_voba_rich_csv(csv: &str) -> Result<Vec<PaymentRecord>> {
    let csv_records_suffix = ";;;;;;;;;;;;;";
    let start = VobaRichPaymentRecord::find_start(csv)
        .ok_or_else(|| anyhow!("Title row in csv did not match:\n\n{}", csv))?;
    let end = csv[start..].find(csv_records_suffix).ok_or_else(|| {
        anyhow!(
            "Found no valid end sequence in uploaded csv:\n\n{}",
            &csv[start..]
        )
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(&csv.as_bytes()[start..(start + end)]);

    let mut result = Vec::new();
    for record in reader.deserialize() {
        let record: VobaRichPaymentRecord = record?;
        result.push(record.into());
    }
    Ok(result)
}

// impl for voba csv file without header and footer

#[derive(Deserialize)]
struct VobaClassicPaymentRecord {
    #[serde(
        rename = "Buchungstag",
        deserialize_with = "deserialize_date_with_german_format"
    )]
    date: NaiveDate,
    #[serde(rename = "Name Zahlungsbeteiligter")]
    payee: String,
    #[serde(rename = "IBAN Zahlungsbeteiligter")]
    payee_iban: String,
    #[serde(rename = "Verwendungszweck")]
    purpose: String,
    #[serde(rename = "Betrag", deserialize_with = "deserialize_float_with_comma")]
    volumne: BigDecimal,
}

impl From<VobaClassicPaymentRecord> for PaymentRecord {
    fn from(record: VobaClassicPaymentRecord) -> Self {
        PaymentRecord::new(
            record.date,
            record.payee,
            record.payee_iban,
            record.purpose,
            record.volumne,
        )
    }
}

fn read_voba_classic_csv(csv: &str) -> Result<Vec<PaymentRecord>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(csv.as_bytes());

    let mut result = Vec::new();
    for record in reader.deserialize() {
        let record: VobaClassicPaymentRecord = record?;
        result.push(record.into());
    }
    Ok(result)
}

// special serde deserializer

fn deserialize_float_with_comma<'de, D>(deserializer: D) -> Result<BigDecimal, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer)?
        .parse_euro_without_symbol()
        .map_err(serde::de::Error::custom)
}

fn deserialize_date_with_german_format<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    NaiveDate::parse_from_str(&string, "%d.%m.%Y").map_err(serde::de::Error::custom)
}

pub(crate) fn write_membership_application(
    membership_application: MembershipApplication,
) -> Result<String> {
    let mut buffer = Vec::new();
    {
        let mut wtr = csv::WriterBuilder::new()
            .delimiter(b';')
            .from_writer(&mut buffer);
        let record: MembershipApplicationRecord = membership_application.into();
        wtr.serialize(record)?;
        wtr.flush()?;
    }
    Ok(String::from_utf8(buffer)?)
}

#[derive(Debug, Serialize)]
struct MembershipApplicationRecord {
    #[serde(rename = "Anrede")]
    salutation: String,

    #[serde(rename = "Vorname")]
    first_name: String,

    #[serde(rename = "Nachname")]
    last_name: String,

    #[serde(rename = "Straße")]
    street: String,

    #[serde(rename = "PLZ")]
    zipcode: String,

    #[serde(rename = "Ort")]
    city: String,

    #[serde(rename = "Land")]
    country: String,

    #[serde(rename = "Geschlecht")]
    gender: String,

    #[serde(rename = "Geburtsdatum")]
    birthday: String,

    #[serde(rename = "Eintrittsdatum")]
    start_date: String,

    #[serde(rename = "Zahlungsart")]
    payment_method: String,

    #[serde(rename = "IBAN")]
    iban: String,

    #[serde(rename = "Kontoinhaber")]
    account_owner: String,

    #[serde(rename = "Status")]
    status: String,

    #[serde(rename = "KommE-Mail_P1")]
    email: String,

    #[serde(rename = "KommTelefon_P1")]
    phone: String,

    #[serde(rename = "Abteilung_1")]
    department: String,

    #[serde(rename = "Abteilungseintritt_1")]
    department_entry: String,

    #[serde(rename = "Beitragsbezeichnung_1_2")]
    membership_type: String,
}

impl From<MembershipApplication> for MembershipApplicationRecord {
    fn from(value: MembershipApplication) -> Self {
        // entry date should be always 01.01.YYYY
        let department_entry = if value.start_date.day() == 1 && value.start_date.month() == 1 {
            value.start_date
        } else {
            NaiveDate::from_ymd_opt(value.start_date.year() + 1, 1, 1).unwrap_or(value.start_date)
        };
        MembershipApplicationRecord {
            salutation: value.salutation,
            first_name: value.first_name,
            last_name: value.last_name,
            street: value.street,
            zipcode: value.zipcode,
            city: value.city,
            country: "Deutschland".into(),
            gender: value.gender,
            birthday: value.birthday,
            start_date: value.start_date.format("%d.%m.%Y").to_string(),
            payment_method: "Lastschrift".into(),
            iban: value.iban,
            account_owner: value.account_owner,
            status: "Aktiv".into(),
            email: value.email,
            phone: value.phone,
            department: value.membership_type.get_department().into(),
            department_entry: department_entry.format("%d.%m.%Y").to_string(),
            membership_type: value.membership_type.get_label().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::models::MembershipType;

    use super::*;
    use bigdecimal::FromPrimitive;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_voba_rich_csv_success() {
        let csv = ";;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Umsatzanzeige;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
BLZ:;10517962;;Datum:;12.03.2022;;;;;;;;;;;;
Konto:;25862911;;Uhrzeit:;14:17:19;;;;;;;;;;;;
Abfrage von:;Paul Ehrlich;;Kontoinhaber:;Sportverein Eutingen im Gäu e.V;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Zeitraum:;;von:;01.03.2022;bis:;12.03.2022;;;;;;;;;;;
Betrag in Euro:;;von:;;bis:;;;;;;;;;;;;
Primanotanummer:;;von:;;bis:;;;;;;;;;;;;
Textschlüssel:;;von:;;bis:;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
09.03.2022;09.03.2022;16 Euro-Überweisung;801;Test GmbH;0;DE92500105174132432988;58629112;GENODES1VBH;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;;EUR;24,15;S
09.03.2022;09.03.2022;51 Überweisungsgutschr.;931;Max Mustermann;0;DE62500105176261449571;10517962;SOLADES1FDS;22-1423 22-1154;;EUR;27,00;H
10.03.2022;10.03.2022;54 Überweisungsgutschr.;932;Erika Mustermann;0;DE91500105176171781279;10517962;SOLADES1FDS;Erika 22-1425 Mustermann;;EUR;33,50;H
10.03.2022;10.03.2022;78 Euro-Überweisung;941;Lieschen Müller;0;DE21500105179625862911;10517962;GENODES1VBH;Lieschen Müller 22-1456;;EUR;27,00;S
10.03.2022;10.03.2022;90 Euro-Überweisung;951;Otto Normalverbraucher;0;DE21500105179625862911;10517962;GENODES1VBH;Otto Normalverbraucher, Test-Kurs,22-1467;;EUR;45,90;H
;;;;;;;;;;;;;
01.03.2022;;;;;;;;;;Anfangssaldo;EUR;10.000,00;H
09.03.2022;;;;;;;;;;Endsaldo;EUR;20.000,00;H
";

        assert_eq!(
            read_voba_rich_csv(csv).unwrap(),
            vec![
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 9).unwrap(),
                    payee: String::from("Test GmbH"),
                    payee_iban: String::from("DE92500105174132432988"),
                    purpose: String::from(
                        "Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH"
                    ),
                    volumne: BigDecimal::from_str("-24.15").unwrap(),
                    payment_ids: HashSet::new(),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 9).unwrap(),
                    payee: String::from("Max Mustermann"),
                    payee_iban: String::from("DE62500105176261449571"),
                    purpose: String::from("22-1423 22-1154"),
                    volumne: BigDecimal::from_i8(27).unwrap(),
                    payment_ids: HashSet::from([String::from("22-1423"), String::from("22-1154")]),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 10).unwrap(),
                    payee: String::from("Erika Mustermann"),
                    payee_iban: String::from("DE91500105176171781279"),
                    purpose: String::from("Erika 22-1425 Mustermann"),
                    volumne: BigDecimal::from_str("33.50").unwrap(),
                    payment_ids: HashSet::from([String::from("22-1425")]),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 10).unwrap(),
                    payee: String::from("Lieschen Müller"),
                    payee_iban: String::from("DE21500105179625862911"),
                    purpose: String::from("Lieschen Müller 22-1456"),
                    volumne: BigDecimal::from_i8(-27).unwrap(),
                    payment_ids: HashSet::from([String::from("22-1456")]),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 10).unwrap(),
                    payee: String::from("Otto Normalverbraucher"),
                    payee_iban: String::from("DE21500105179625862911"),
                    purpose: String::from("Otto Normalverbraucher, Test-Kurs,22-1467"),
                    volumne: BigDecimal::from_str("45.90").unwrap(),
                    payment_ids: HashSet::from([String::from("22-1467")]),
                },
            ]
        );
    }

    #[test]
    fn test_voba_rich_csv_error() {
        let mut csv = "Valuta;Textschlüssel";

        assert_eq!(
            format!("{}", read_voba_rich_csv(csv).unwrap_err()),
            "Title row in csv did not match:

Valuta;Textschlüssel"
        );

        csv = "Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
;;;;;";

        assert_eq!(
            format!("{}", read_voba_rich_csv(csv).unwrap_err()),
            "Found no valid end sequence in uploaded csv:

Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
;;;;;"  
        );
    }

    #[test]
    fn test_voba_classic_csv_success() {
        let csv = "Bezeichnung Auftragskonto;IBAN Auftragskonto;BIC Auftragskonto;Bankname Auftragskonto;Buchungstag;Valutadatum;Name Zahlungsbeteiligter;IBAN Zahlungsbeteiligter;BIC (SWIFT-Code) Zahlungsbeteiligter;Buchungstext;Verwendungszweck;Betrag;Waehrung;Saldo nach Buchung;Bemerkung;Kategorie;Steuerrelevant;Glaeubiger ID;Mandatsreferenz
Festgeldkonto (Tagesgeld);DE68500105173456568557;GENODES1FDS;VOLKSBANK IM KREIS FREUDENSTADT;09.03.2022;09.03.2022;Test GmbH;DE92500105174132432988;GENODES1VBH;16 Euro-Überweisung;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;-24,15;EUR;260,00;;;;;
Festgeldkonto (Tagesgeld);DE68500105173456568557;GENODES1FDS;VOLKSBANK IM KREIS FREUDENSTADT;09.03.2022;09.03.2022;Max Mustermann;DE62500105176261449571;SOLADES1FDS;Überweisungsgutschr.;22-1423;27,00;EUR;152,00;;;;;
Festgeldkonto (Tagesgeld);DE68500105173456568557;GENODES1FDS;VOLKSBANK IM KREIS FREUDENSTADT;10.03.2022;10.03.2022;Erika Mustermann;DE91500105176171781279;SOLADES1FDS;Überweisungsgutschr.;Erika 22-1425 Mustermann;33,50;EUR;98,00;;;;;
Festgeldkonto (Tagesgeld);DE68500105173456568557;GENODES1FDS;VOLKSBANK IM KREIS FREUDENSTADT;10.03.2022;10.03.2022;Lieschen Müller;DE21500105179625862911;GENODES1VBH;Überweisungsgutschr.;Lieschen Müller 22-1456;-27,00;EUR;54,00;;;;;
Festgeldkonto (Tagesgeld);DE68500105173456568557;GENODES1FDS;VOLKSBANK IM KREIS FREUDENSTADT;10.03.2022;10.03.2022;Otto Normalverbraucher;DE21500105179625862911;GENODES1VBH;Überweisungsgutschr.;Otto Normalverbraucher, Test-Kurs,22-1467;45,90;EUR;0,00;;;;;
";

        assert_eq!(
            read_voba_classic_csv(csv).unwrap(),
            vec![
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 9).unwrap(),
                    payee: String::from("Test GmbH"),
                    payee_iban: String::from("DE92500105174132432988"),
                    purpose: String::from(
                        "Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH"
                    ),
                    volumne: BigDecimal::from_str("-24.15").unwrap(),
                    payment_ids: HashSet::new(),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 9).unwrap(),
                    payee: String::from("Max Mustermann"),
                    payee_iban: String::from("DE62500105176261449571"),
                    purpose: String::from("22-1423"),
                    volumne: BigDecimal::from_i8(27).unwrap(),
                    payment_ids: HashSet::from([String::from("22-1423")]),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 10).unwrap(),
                    payee: String::from("Erika Mustermann"),
                    payee_iban: String::from("DE91500105176171781279"),
                    purpose: String::from("Erika 22-1425 Mustermann"),
                    volumne: BigDecimal::from_str("33.50").unwrap(),
                    payment_ids: HashSet::from([String::from("22-1425")]),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 10).unwrap(),
                    payee: String::from("Lieschen Müller"),
                    payee_iban: String::from("DE21500105179625862911"),
                    purpose: String::from("Lieschen Müller 22-1456"),
                    volumne: BigDecimal::from_i8(-27).unwrap(),
                    payment_ids: HashSet::from([String::from("22-1456")]),
                },
                PaymentRecord {
                    date: NaiveDate::from_ymd_opt(2022, 3, 10).unwrap(),
                    payee: String::from("Otto Normalverbraucher"),
                    payee_iban: String::from("DE21500105179625862911"),
                    purpose: String::from("Otto Normalverbraucher, Test-Kurs,22-1467"),
                    volumne: BigDecimal::from_str("45.90").unwrap(),
                    payment_ids: HashSet::from([String::from("22-1467")]),
                }
            ]
        );
    }

    #[test]
    fn test_voba_classic_csv_error() {
        let csv = ";;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Umsatzanzeige;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
BLZ:;10517962;;Datum:;12.03.2022;;;;;;;;;;;;
Konto:;25862911;;Uhrzeit:;14:17:19;;;;;;;;;;;;
Abfrage von:;Paul Ehrlich;;Kontoinhaber:;Sportverein Eutingen im Gäu e.V;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Zeitraum:;;von:;01.03.2022;bis:;12.03.2022;;;;;;;;;;;
Betrag in Euro:;;von:;;bis:;;;;;;;;;;;;
Primanotanummer:;;von:;;bis:;;;;;;;;;;;;
Textschlüssel:;;von:;;bis:;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
09.03.2022;09.03.2022;16 Euro-Überweisung;801;Test GmbH;0;DE92500105174132432988;58629112;GENODES1VBH;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;;EUR;24,15;S
;;;;;;;;;;;;;
01.03.2022;;;;;;;;;;Anfangssaldo;EUR;10.000,00;H
09.03.2022;;;;;;;;;;Endsaldo;EUR;20.000,00;H
";
        assert!(read_voba_classic_csv(csv).is_err());
    }

    #[test]
    fn test_write_membership_application() {
        let application = MembershipApplication {
            salutation: "Herr".into(),
            first_name: "Max".into(),
            last_name: "Mustermann".into(),
            street: "Musterstraße 10".into(),
            zipcode: "12345".into(),
            city: "Musterstadt".into(),
            email: "max.mustermann@example.com".into(),
            phone: "0123456789".into(),
            gender: "männlich".into(),
            birthday: "01.01.1970".into(),
            start_date: NaiveDate::from_ymd_opt(2022, 9, 6).unwrap(),
            iban: "DE92500105174132432988".into(),
            account_owner: "Max Mustermann".into(),
            membership_type: MembershipType::AdultPremium,
            family_members: None,
            newsletter: false,
            token: None,
        };

        let csv = write_membership_application(application).unwrap();
        assert_eq!(
            csv,
            r#"Anrede;Vorname;Nachname;Straße;PLZ;Ort;Land;Geschlecht;Geburtsdatum;Eintrittsdatum;Zahlungsart;IBAN;Kontoinhaber;Status;KommE-Mail_P1;KommTelefon_P1;Abteilung_1;Abteilungseintritt_1;Beitragsbezeichnung_1_2
Herr;Max;Mustermann;Musterstraße 10;12345;Musterstadt;Deutschland;männlich;01.01.1970;06.09.2022;Lastschrift;DE92500105174132432988;Max Mustermann;Aktiv;max.mustermann@example.com;0123456789;Hauptverein;01.01.2023;Premiummitglied Erwachsener
"#
        );

        let application = MembershipApplication {
            salutation: "Herr".into(),
            first_name: "Max".into(),
            last_name: "Mustermann".into(),
            street: "Musterstraße 10".into(),
            zipcode: "12345".into(),
            city: "Musterstadt".into(),
            email: "max.mustermann@example.com".into(),
            phone: "0123456789".into(),
            gender: "männlich".into(),
            birthday: "01.01.1970".into(),
            start_date: NaiveDate::from_ymd_opt(2022, 1, 1).unwrap(),
            iban: "DE92500105174132432988".into(),
            account_owner: "Max Mustermann".into(),
            membership_type: MembershipType::AdultPremium,
            family_members: None,
            newsletter: false,
            token: None,
        };

        let csv = write_membership_application(application).unwrap();
        assert_eq!(
            csv,
            r#"Anrede;Vorname;Nachname;Straße;PLZ;Ort;Land;Geschlecht;Geburtsdatum;Eintrittsdatum;Zahlungsart;IBAN;Kontoinhaber;Status;KommE-Mail_P1;KommTelefon_P1;Abteilung_1;Abteilungseintritt_1;Beitragsbezeichnung_1_2
Herr;Max;Mustermann;Musterstraße 10;12345;Musterstadt;Deutschland;männlich;01.01.1970;01.01.2022;Lastschrift;DE92500105174132432988;Max Mustermann;Aktiv;max.mustermann@example.com;0123456789;Hauptverein;01.01.2022;Premiummitglied Erwachsener
"#
        );
    }
}
