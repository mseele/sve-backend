use crate::error::ValidationError;
use crate::models::MembershipApplication;
use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate};
use rust_xlsxwriter::{Workbook, Worksheet};
use uuid::Uuid;

/// Header row of the DVH ("Digitales Vereinsheim") profile import template.
/// Wording and order have to match the template exactly.
const HEADERS: [&str; 37] = [
    "Vorname",
    "Nachname",
    "Geschlecht",
    "Geburtstag",
    "Nationalität",
    "Sprache",
    "E-Mail",
    "Zweite E-Mail",
    "Telefon",
    "Zweite Telefonnummer",
    "Adresse",
    "Adresszusatz",
    "Postleitzahl",
    "Stadt",
    "Bundesland",
    "Land",
    "Externe ID",
    "Notfallkontakt",
    "Notfallkontakt Telefon",
    "Typ",
    "Mitgliedsnummer",
    "Mitgliedschaftsbeginn",
    "Mitgliedschaftsende",
    "Mitgliedschaftstyp",
    "Abteilung Fitness Startdatum",
    "Abteilung Fitness Enddatum",
    "Abteilung Hauptverein Startdatum",
    "Abteilung Hauptverein Enddatum",
    "Abteilung Volleyball Startdatum",
    "Abteilung Volleyball Enddatum",
    "DFBnet Beiträge",
    "Zahlungsmethode",
    "SEPA IBAN",
    "SEPA Kontoinhaber",
    "SEPA Mandats-ID",
    "SEPA Unterschriftsdatum",
    "Benutzerdefiniertes Feld Debitorenkonto-Nr.",
];

// positions of the filled columns, following the header order above
const FIRST_NAME: u16 = 0;
const LAST_NAME: u16 = 1;
const GENDER: u16 = 2;
const BIRTHDAY: u16 = 3;
const LANGUAGE: u16 = 5;
const EMAIL: u16 = 6;
const PHONE: u16 = 8;
const ADDRESS: u16 = 10;
const ZIPCODE: u16 = 12;
const CITY: u16 = 13;
const COUNTRY: u16 = 15;
const MEMBERSHIP_NUMBER: u16 = 20;
const MEMBERSHIP_START: u16 = 21;
const MEMBERSHIP_TYPE: u16 = 23;
const DEPARTMENT_FITNESS_START: u16 = 24;
const DEPARTMENT_MAIN_CLUB_START: u16 = 26;
const DFBNET_DUES: u16 = 30;
const PAYMENT_METHOD: u16 = 31;
const SEPA_IBAN: u16 = 32;
const SEPA_ACCOUNT_OWNER: u16 = 33;
const SEPA_MANDATE_ID: u16 = 34;
const SEPA_SIGNATURE_DATE: u16 = 35;

/// One person row of the import file.
struct PersonRow<'a> {
    first_name: &'a str,
    last_name: &'a str,
    gender: Option<&'static str>,
    birthday: NaiveDate,
}

/// Parse the birthdays of the applicant and all family members in row order
/// (applicant first). Rejects the application with a validation error if a
/// birthday does not parse as DD.MM.YYYY, so that bad data never reaches the
/// import file.
pub(crate) fn parse_birthdays(application: &MembershipApplication) -> Result<Vec<NaiveDate>> {
    let mut birthdays = vec![parse_birthday(
        &application.birthday,
        &application.first_name,
    )?];
    if let Some(family_members) = &application.family_members {
        for member in family_members {
            birthdays.push(parse_birthday(&member.birthday, &member.first_name)?);
        }
    }
    Ok(birthdays)
}

fn parse_birthday(birthday: &str, first_name: &str) -> Result<NaiveDate> {
    if let Ok(date) = NaiveDate::parse_from_str(birthday, "%d.%m.%Y") {
        return Ok(date);
    }
    bail!(ValidationError::new(format!(
        "Bitte gib für {first_name} ein gültiges Geburtsdatum im Format TT.MM.JJJJ ein."
    )))
}

/// Build the application as an xlsx in the DVH profile import format: sheet
/// "Template" with the template header row and one row per person, the
/// applicant first, followed by the family members.
pub(crate) fn write_membership_application(
    application: &MembershipApplication,
    application_date: NaiveDate,
) -> Result<Vec<u8>> {
    let birthdays = parse_birthdays(application)?;

    // one random suffix per application, shared by the mandate id and all
    // membership numbers of this application
    let suffix = random_suffix();
    let mandate_id = mandate_id(application_date, &suffix);

    let mut persons = vec![PersonRow {
        first_name: &application.first_name,
        last_name: &application.last_name,
        gender: Some(map_gender(&application.gender)),
        birthday: birthdays[0],
    }];
    if let Some(family_members) = &application.family_members {
        for (member, birthday) in family_members.iter().zip(&birthdays[1..]) {
            // gender is not collected for family members
            persons.push(PersonRow {
                first_name: &member.first_name,
                last_name: &member.last_name,
                gender: None,
                birthday: *birthday,
            });
        }
    }

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Template")?;
    for (col, header) in HEADERS.iter().enumerate() {
        sheet.write(0, col as u16, *header)?;
    }
    for (index, person) in persons.iter().enumerate() {
        write_person_row(
            sheet,
            (index + 1) as u32,
            person,
            &membership_number(application_date, &suffix, index),
            application,
            &mandate_id,
            application_date,
        )?;
    }

    Ok(workbook.save_to_buffer()?)
}

fn write_person_row(
    sheet: &mut Worksheet,
    row: u32,
    person: &PersonRow,
    membership_number: &str,
    application: &MembershipApplication,
    mandate_id: &str,
    application_date: NaiveDate,
) -> Result<()> {
    // only the department pair of the membership type is filled, the
    // other pairs stay present but empty
    let fitness = application.membership_type.get_department() == "Fitness";
    let department_start = department_start_date(application.start_date);

    sheet.write(row, FIRST_NAME, person.first_name)?;
    sheet.write(row, LAST_NAME, person.last_name)?;
    if let Some(gender) = person.gender {
        sheet.write(row, GENDER, gender)?;
    }
    sheet.write(
        row,
        BIRTHDAY,
        person.birthday.format("%d.%m.%Y").to_string(),
    )?;
    sheet.write(row, LANGUAGE, "de")?;
    sheet.write(row, EMAIL, &application.email)?;
    sheet.write(row, PHONE, &application.phone)?;
    sheet.write(row, ADDRESS, &application.street)?;
    sheet.write(row, ZIPCODE, &application.zipcode)?;
    sheet.write(row, CITY, &application.city)?;
    sheet.write(row, COUNTRY, "DEU")?;
    sheet.write(row, MEMBERSHIP_NUMBER, membership_number)?;
    sheet.write(
        row,
        MEMBERSHIP_START,
        application.start_date.format("%d.%m.%Y").to_string(),
    )?;
    sheet.write(
        row,
        MEMBERSHIP_TYPE,
        application.membership_type.get_dvh_membership_type(),
    )?;
    if fitness {
        sheet.write(
            row,
            DEPARTMENT_FITNESS_START,
            department_start.format("%d.%m.%Y").to_string(),
        )?;
    } else {
        sheet.write(
            row,
            DEPARTMENT_MAIN_CLUB_START,
            department_start.format("%d.%m.%Y").to_string(),
        )?;
    }
    sheet.write(row, DFBNET_DUES, application.membership_type.get_dfb_due())?;
    sheet.write(row, PAYMENT_METHOD, "SEPA-Lastschrift")?;
    sheet.write(row, SEPA_IBAN, &application.iban)?;
    sheet.write(row, SEPA_ACCOUNT_OWNER, &application.account_owner)?;
    // the mandate id is shared by all rows of the application
    sheet.write(row, SEPA_MANDATE_ID, mandate_id)?;
    sheet.write(
        row,
        SEPA_SIGNATURE_DATE,
        application_date.format("%d.%m.%Y").to_string(),
    )?;

    Ok(())
}

/// Gender mapping for the DVH import.
fn map_gender(gender: &str) -> &'static str {
    match gender {
        "männlich" => "M",
        "weiblich" => "W",
        _ => "U",
    }
}

/// Department entry date is always a first of January: the start date itself
/// if it is one, otherwise the following one.
fn department_start_date(start_date: NaiveDate) -> NaiveDate {
    if start_date.day() == 1 && start_date.month() == 1 {
        start_date
    } else {
        NaiveDate::from_ymd_opt(start_date.year() + 1, 1, 1).unwrap_or(start_date)
    }
}

/// SEPA mandate id of the application, shared by all of its rows. The `SVEUT-W`
/// prefix separates website-generated mandates from the `SVEUT-` scheme DVH
/// assigns itself.
fn mandate_id(application_date: NaiveDate, suffix: &str) -> String {
    format!("SVEUT-W-{}-{suffix}", application_date.format("%Y%m%d"))
}

/// Membership number of the person in the given row. All persons of an
/// application share the suffix, family members get an additional row index.
fn membership_number(application_date: NaiveDate, suffix: &str, index: usize) -> String {
    let base = format!("WS-{}-{suffix}", application_date.format("%Y%m%d"));
    if index == 0 {
        base
    } else {
        format!("{base}-{}", index + 1)
    }
}

/// Four random hex chars, drawn once per application.
fn random_suffix() -> String {
    let uuid = Uuid::new_v4().simple().to_string();
    uuid[..4].to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::io::Cursor;

    use crate::models::{MembershipApplication, MembershipFamilyMember, MembershipType};

    use super::*;
    use calamine::{DataType, Range, Reader, Xlsx};
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use regex::Regex;

    fn application(
        membership_type: MembershipType,
        family_members: Option<Vec<MembershipFamilyMember>>,
    ) -> MembershipApplication {
        MembershipApplication {
            salutation: "Herr".into(),
            first_name: "Max".into(),
            last_name: "Mustermann".into(),
            street: "Musterstraße 1".into(),
            zipcode: "12345".into(),
            city: "Musterstadt".into(),
            email: "max@example.com".into(),
            phone: "0123456789".into(),
            gender: "männlich".into(),
            birthday: "01.01.1970".into(),
            start_date: NaiveDate::from_ymd_opt(2022, 9, 6).unwrap(),
            iban: "DE89370400440532013000".into(),
            account_owner: "Max Mustermann".into(),
            membership_type,
            family_members,
            newsletter: false,
            token: None,
        }
    }

    fn family_member(first_name: &str, birthday: &str) -> MembershipFamilyMember {
        MembershipFamilyMember {
            first_name: first_name.into(),
            last_name: "Mustermann".into(),
            birthday: birthday.into(),
        }
    }

    fn read_sheet(bytes: Vec<u8>) -> Range<calamine::Data> {
        let mut workbook: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).unwrap();
        workbook.worksheet_range("Template").unwrap()
    }

    fn cell(range: &Range<calamine::Data>, row: usize, col: usize) -> String {
        range
            .get((row, col))
            .and_then(|data| data.as_string())
            .unwrap_or_default()
    }

    #[test]
    fn test_headers_match_dvh_template() {
        assert_eq!(
            HEADERS,
            [
                "Vorname",
                "Nachname",
                "Geschlecht",
                "Geburtstag",
                "Nationalität",
                "Sprache",
                "E-Mail",
                "Zweite E-Mail",
                "Telefon",
                "Zweite Telefonnummer",
                "Adresse",
                "Adresszusatz",
                "Postleitzahl",
                "Stadt",
                "Bundesland",
                "Land",
                "Externe ID",
                "Notfallkontakt",
                "Notfallkontakt Telefon",
                "Typ",
                "Mitgliedsnummer",
                "Mitgliedschaftsbeginn",
                "Mitgliedschaftsende",
                "Mitgliedschaftstyp",
                "Abteilung Fitness Startdatum",
                "Abteilung Fitness Enddatum",
                "Abteilung Hauptverein Startdatum",
                "Abteilung Hauptverein Enddatum",
                "Abteilung Volleyball Startdatum",
                "Abteilung Volleyball Enddatum",
                "DFBnet Beiträge",
                "Zahlungsmethode",
                "SEPA IBAN",
                "SEPA Kontoinhaber",
                "SEPA Mandats-ID",
                "SEPA Unterschriftsdatum",
                "Benutzerdefiniertes Feld Debitorenkonto-Nr.",
            ]
        );
    }

    #[test]
    fn test_parse_birthdays() {
        let application = application(
            MembershipType::Family,
            Some(vec![family_member("Anna", "15.03.1960")]),
        );
        let birthdays = parse_birthdays(&application).unwrap();
        assert_eq!(
            birthdays,
            vec![
                NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(1960, 3, 15).unwrap(),
            ]
        );
    }

    #[test]
    fn test_parse_birthdays_rejects_invalid_applicant_birthday() {
        let mut application = application(MembershipType::AdultActive, None);
        application.birthday = "1970-01-01".into();
        let error = parse_birthdays(&application).unwrap_err().to_string();
        assert!(error.contains("Max"));
        assert!(error.contains("Geburtsdatum"));
    }

    #[test]
    fn test_parse_birthdays_rejects_invalid_family_birthday() {
        let application = application(
            MembershipType::Family,
            Some(vec![family_member("Anna", "32.13.1960")]),
        );
        let error = parse_birthdays(&application).unwrap_err().to_string();
        assert!(error.contains("Anna"));
        assert!(error.contains("Geburtsdatum"));
    }

    #[test]
    fn test_map_gender() {
        assert_eq!(map_gender("männlich"), "M");
        assert_eq!(map_gender("weiblich"), "W");
        assert_eq!(map_gender("divers"), "U");
        assert_eq!(map_gender(""), "U");
    }

    #[test]
    fn test_department_start_date() {
        assert_eq!(
            department_start_date(NaiveDate::from_ymd_opt(2022, 1, 1).unwrap()),
            NaiveDate::from_ymd_opt(2022, 1, 1).unwrap()
        );
        assert_eq!(
            department_start_date(NaiveDate::from_ymd_opt(2022, 9, 6).unwrap()),
            NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()
        );
        assert_eq!(
            department_start_date(NaiveDate::from_ymd_opt(2022, 12, 31).unwrap()),
            NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()
        );
    }

    #[test]
    fn test_mandate_id_format() {
        let application_date = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        assert_eq!(
            mandate_id(application_date, "4f2a"),
            "SVEUT-W-20261107-4f2a"
        );
    }

    #[test]
    fn test_membership_number_format() {
        let application_date = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        assert_eq!(
            membership_number(application_date, "4f2a", 0),
            "WS-20261107-4f2a"
        );
        assert_eq!(
            membership_number(application_date, "4f2a", 1),
            "WS-20261107-4f2a-2"
        );
        assert_eq!(
            membership_number(application_date, "4f2a", 2),
            "WS-20261107-4f2a-3"
        );
    }

    #[test]
    fn test_write_membership_application_round_trip() {
        let application_date = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let bytes = write_membership_application(
            &application(MembershipType::AdultActive, None),
            application_date,
        )
        .unwrap();
        let range = read_sheet(bytes);

        // header row
        for (col, header) in HEADERS.iter().enumerate() {
            assert_eq!(cell(&range, 0, col), *header);
        }
        // exactly one data row
        assert_eq!(range.get_size(), (2, 37));

        assert_eq!(cell(&range, 1, 0), "Max");
        assert_eq!(cell(&range, 1, 1), "Mustermann");
        assert_eq!(cell(&range, 1, 2), "M");
        assert_eq!(cell(&range, 1, 3), "01.01.1970");
        assert_eq!(cell(&range, 1, 5), "de");
        assert_eq!(cell(&range, 1, 6), "max@example.com");
        assert_eq!(cell(&range, 1, 8), "0123456789");
        assert_eq!(cell(&range, 1, 10), "Musterstraße 1");
        assert_eq!(cell(&range, 1, 12), "12345");
        assert_eq!(cell(&range, 1, 13), "Musterstadt");
        assert_eq!(cell(&range, 1, 15), "DEU");
        let membership_number = cell(&range, 1, 20);
        assert!(
            Regex::new(r"^WS-20261107-[0-9a-f]{4}$")
                .unwrap()
                .is_match(&membership_number)
        );
        assert_eq!(cell(&range, 1, 21), "06.09.2022");
        assert_eq!(cell(&range, 1, 23), "Active");
        assert_eq!(cell(&range, 1, 24), "");
        // Jan-1 rule: 06.09.2022 -> 01.01.2023
        assert_eq!(cell(&range, 1, 26), "01.01.2023");
        assert_eq!(cell(&range, 1, 30), "Aktiver Erwachsener Jahr");
        assert_eq!(cell(&range, 1, 31), "SEPA-Lastschrift");
        assert_eq!(cell(&range, 1, 32), "DE89370400440532013000");
        assert_eq!(cell(&range, 1, 33), "Max Mustermann");
        let mandate_id = cell(&range, 1, 34);
        assert!(
            Regex::new(r"^SVEUT-W-20261107-[0-9a-f]{4}$")
                .unwrap()
                .is_match(&mandate_id)
        );
        // the mandate id shares the suffix of the membership number
        assert_eq!(
            mandate_id,
            format!("SVEUT-W-20261107-{}", &membership_number[12..])
        );
        assert_eq!(cell(&range, 1, 35), "07.11.2026");

        // uncollected columns stay empty
        for col in [4, 7, 9, 11, 14, 16, 17, 18, 19, 22, 25, 27, 28, 29, 36] {
            assert_eq!(cell(&range, 1, col), "");
        }
    }

    #[test]
    fn test_write_membership_application_family_round_trip() {
        let application_date = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let application = application(
            MembershipType::Family,
            Some(vec![
                family_member("Anna", "15.03.1960"),
                family_member("Ben", "20.05.2018"),
            ]),
        );
        let bytes = write_membership_application(&application, application_date).unwrap();
        let range = read_sheet(bytes);
        assert_eq!(range.get_size(), (4, 37));

        // values shared by every row, including family dues on the parent row
        for row in 1..=3 {
            assert_eq!(
                cell(&range, row, 30),
                "Familienbeitrag beliebige Kinder Jahr"
            );
            assert_eq!(cell(&range, row, 23), "Active");
            // family rows use the parent's email
            assert_eq!(cell(&range, row, 6), "max@example.com");
            assert_eq!(cell(&range, row, 15), "DEU");
            assert_eq!(cell(&range, row, 21), "06.09.2022");
        }

        // parent row
        assert_eq!(cell(&range, 1, 0), "Max");
        assert_eq!(cell(&range, 1, 2), "M");

        // family rows: own names and birthdays, no gender
        assert_eq!(cell(&range, 2, 0), "Anna");
        assert_eq!(cell(&range, 2, 1), "Mustermann");
        assert_eq!(cell(&range, 2, 2), "");
        assert_eq!(cell(&range, 2, 3), "15.03.1960");
        assert_eq!(cell(&range, 3, 0), "Ben");
        assert_eq!(cell(&range, 3, 3), "20.05.2018");

        // membership numbers are unique and share the suffix of the parent
        let numbers = [
            cell(&range, 1, 20),
            cell(&range, 2, 20),
            cell(&range, 3, 20),
        ];
        let unique_numbers: HashSet<_> = numbers.iter().collect();
        assert_eq!(unique_numbers.len(), 3);
        assert_eq!(numbers[1], format!("{}-2", numbers[0]));
        assert_eq!(numbers[2], format!("{}-3", numbers[0]));

        // one mandate id shared by all rows
        let mandate_ids: HashSet<_> = (1..=3).map(|row| cell(&range, row, 34)).collect();
        assert_eq!(mandate_ids.len(), 1);
    }

    #[test]
    fn test_write_membership_application_type_specific_columns() {
        let application_date = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();

        // Fördermitglied is the only passive membership type
        let bytes = write_membership_application(
            &application(MembershipType::AdultSuporting, None),
            application_date,
        )
        .unwrap();
        let range = read_sheet(bytes);
        assert_eq!(cell(&range, 1, 23), "Passive");
        assert_eq!(cell(&range, 1, 30), "Fördermitglied Erwachsener Jahr");
        assert_eq!(cell(&range, 1, 26), "01.01.2023");
        assert_eq!(cell(&range, 1, 24), "");

        // fitness fills the fitness pair instead of the Hauptverein pair
        let bytes = write_membership_application(
            &application(MembershipType::Fitness, None),
            application_date,
        )
        .unwrap();
        let range = read_sheet(bytes);
        assert_eq!(cell(&range, 1, 23), "Active");
        assert_eq!(cell(&range, 1, 30), "Sparte Fitness Jahr");
        assert_eq!(cell(&range, 1, 24), "01.01.2023");
        assert_eq!(cell(&range, 1, 26), "");
    }

    #[test]
    #[ignore = "writes a dummy import file for a manual dry run against the DVH import validator"]
    fn generate_dry_run_file() {
        let application = MembershipApplication {
            salutation: "Herr".into(),
            first_name: "Paul".into(),
            last_name: "Testfall".into(),
            street: "Testweg 9".into(),
            zipcode: "72184".into(),
            city: "Eutingen".into(),
            email: "paul.testfall@example.com".into(),
            phone: "0171 5551234".into(),
            gender: "männlich".into(),
            birthday: "20.05.1982".into(),
            start_date: NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(),
            iban: "DE89370400440532013000".into(),
            account_owner: "Paul Testfall".into(),
            membership_type: MembershipType::Family,
            family_members: Some(vec![
                MembershipFamilyMember {
                    first_name: "Timo".into(),
                    last_name: "Testfall".into(),
                    birthday: "10.01.2018".into(),
                },
                MembershipFamilyMember {
                    first_name: "Lena".into(),
                    last_name: "Testfall".into(),
                    birthday: "22.07.2021".into(),
                },
            ]),
            newsletter: false,
            token: None,
        };
        let bytes = write_membership_application(
            &application,
            NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
        )
        .unwrap();
        let path = std::env::temp_dir().join("dvh_import_test_v3.xlsx");
        std::fs::write(&path, bytes).unwrap();
        println!("dry run file written to {}", path.display());
    }
}
