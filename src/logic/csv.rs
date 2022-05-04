use crate::models::FromEuro;
use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use serde::Deserialize;

pub fn read(csv: &str) -> Result<Vec<PaymentRecord>> {
    VobaRichCSVReader::read(csv)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaymentRecord {
    pub date: NaiveDate,
    pub payee: String,
    pub payee_iban: String,
    pub purpose: String,
    pub volumne: f64,
}

impl PaymentRecord {
    fn new(
        date: NaiveDate,
        payee: String,
        payee_iban: String,
        purpose: String,
        volumne: f64,
    ) -> Self {
        Self {
            date,
            payee,
            payee_iban,
            purpose,
            volumne,
        }
    }
}

trait CSVReader {
    fn read(csv: &str) -> Result<Vec<PaymentRecord>>;
}

// impl for voba csv file with header and footer

struct VobaRichCSVReader {}

#[derive(Deserialize)]
struct VobaRichPaymentRecord {
    #[serde(
        rename = "Buchungstag",
        deserialize_with = "deserialize_date_with_german_format"
    )]
    date: NaiveDate,
    #[serde(rename = "Valuta")]
    _valuta: String,
    #[serde(rename = "Textschlüssel")]
    _textkey: String,
    #[serde(rename = "Primanota")]
    _primanota: String,
    #[serde(rename = "Zahlungsempfänger")]
    payee: String,
    #[serde(rename = "ZahlungsempfängerKto")]
    _payee_account: String,
    #[serde(rename = "ZahlungsempfängerIBAN")]
    payee_iban: String,
    #[serde(rename = "ZahlungsempfängerBLZ")]
    _payee_blz: String,
    #[serde(rename = "ZahlungsempfängerBIC")]
    _payee_bic: String,
    #[serde(rename = "Vorgang/Verwendungszweck")]
    purpose: String,
    #[serde(rename = "Kundenreferenz")]
    _customer_reference: String,
    #[serde(rename = "Währung")]
    _currency: String,
    #[serde(rename = "Umsatz", deserialize_with = "deserialize_float_with_comma")]
    volumne: f64,
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
    fn volumne(&self) -> f64 {
        match self.debit_credit.as_str() {
            "H" => self.volumne,
            _ => -self.volumne,
        }
    }
}

impl CSVReader for VobaRichCSVReader {
    fn read(csv: &str) -> Result<Vec<PaymentRecord>> {
        let csv_records_prefix = "Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben";
        let csv_records_suffix = ";;;;;;;;;;;;;";
        let start = csv
            .find(csv_records_prefix)
            .ok_or_else(|| anyhow!("Title row in csv did not match:\n\n{}", csv))?;
        let end = csv[start..].find(csv_records_suffix).ok_or_else(|| {
            anyhow!(
                "Found no valid end sequence in uploaded csv:\n\n{}",
                &csv[start..]
            )
        })?;
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .from_reader(csv[start..(start + end)].as_bytes());

        let mut result = Vec::new();
        for record in reader.deserialize() {
            let record: VobaRichPaymentRecord = record?;
            result.push(record.into());
        }
        Ok(result)
    }
}

// special serde deserializer

fn deserialize_float_with_comma<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer)?
        .from_euro_without_symbol()
        .map_err(serde::de::Error::custom)
}

fn deserialize_date_with_german_format<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    NaiveDate::parse_from_str(&string, "%d.%m.%Y").map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
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
09.03.2022;09.03.2022;51 Überweisungsgutschr.;931;Max Mustermann;0;DE62500105176261449571;10517962;SOLADES1FDS;22-1423;;EUR;27,00;H
10.03.2022;10.03.2022;54 Überweisungsgutschr.;932;Erika Mustermann;0;DE91500105176171781279;10517962;SOLADES1FDS;Erika 22-1425 Mustermann;;EUR;33,50;H
10.03.2022;10.03.2022;78 Euro-Überweisung;941;Lieschen Müller;0;DE21500105179625862911;10517962;GENODES1VBH;Lieschen Müller 22-1456;;EUR;27,00;S
10.03.2022;10.03.2022;90 Euro-Überweisung;951;Otto Normalverbraucher;0;DE21500105179625862911;10517962;GENODES1VBH;Otto Normalverbraucher, Test-Kurs,22-1467;;EUR;45,90;H
;;;;;;;;;;;;;
01.03.2022;;;;;;;;;;Anfangssaldo;EUR;10.000,00;H
09.03.2022;;;;;;;;;;Endsaldo;EUR;20.000,00;H
";

        assert_eq!(
            VobaRichCSVReader::read(csv).unwrap(),
            vec![
                PaymentRecord::new(
                    NaiveDate::from_ymd(2022, 3, 9),
                    String::from("Test GmbH"),
                    String::from("DE92500105174132432988"),
                    String::from("Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH"),
                    -24.15
                ),
                PaymentRecord::new(
                    NaiveDate::from_ymd(2022, 3, 9),
                    String::from("Max Mustermann"),
                    String::from("DE62500105176261449571"),
                    String::from("22-1423"),
                    27.00
                ),
                PaymentRecord::new(
                    NaiveDate::from_ymd(2022, 3, 10),
                    String::from("Erika Mustermann"),
                    String::from("DE91500105176171781279"),
                    String::from("Erika 22-1425 Mustermann"),
                    33.50
                ),
                PaymentRecord::new(
                    NaiveDate::from_ymd(2022, 3, 10),
                    String::from("Lieschen Müller"),
                    String::from("DE21500105179625862911"),
                    String::from("Lieschen Müller 22-1456"),
                    -27.00
                ),
                PaymentRecord::new(
                    NaiveDate::from_ymd(2022, 3, 10),
                    String::from("Otto Normalverbraucher"),
                    String::from("DE21500105179625862911"),
                    String::from("Otto Normalverbraucher, Test-Kurs,22-1467"),
                    45.90
                )
            ]
        );
    }

    #[test]
    fn test_voba_rich_csv_error() {
        let mut csv = "Valuta;Textschlüssel";

        assert_eq!(
            format!("{}", VobaRichCSVReader::read(csv).unwrap_err()),
            "Title row in csv did not match:

Valuta;Textschlüssel"
        );

        csv = "Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
;;;;;";

        assert_eq!(
            format!("{}", VobaRichCSVReader::read(csv).unwrap_err()),
            "Found no valid end sequence in uploaded csv:

Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
;;;;;"  
        );
    }
}
