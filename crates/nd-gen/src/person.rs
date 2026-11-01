use crate::{DataLocale, Rng, data, data_en as en, it, plain};

pub struct Person {
    pub locale: DataLocale,
    pub first: &'static str,
    pub last: &'static str,
    female: bool,
    pub born: i64,
    city: usize,
    second: &'static str,
}

fn surnames(l: DataLocale) -> &'static [&'static str] {
    match l {
        DataLocale::It => data::SURNAMES,
        DataLocale::Uk => en::SURNAMES_UK,
        DataLocale::Us => en::SURNAMES_US,
    }
}

fn cities(l: DataLocale) -> usize {
    match l {
        DataLocale::It => data::CITIES.len(),
        DataLocale::Uk => en::CITIES_UK.len(),
        DataLocale::Us => en::CITIES_US.len(),
    }
}

pub fn words(l: DataLocale) -> &'static [&'static str] {
    if l == DataLocale::It {
        data::LOREM
    } else {
        en::LOREM
    }
}

fn digits(r: &mut Rng, n: usize) -> String {
    (0..n).map(|_| char::from(*r.pick(data::DIGITS))).collect()
}

pub fn street(r: &mut Rng, l: DataLocale) -> String {
    match l {
        DataLocale::It => format!(
            "{} {}, {}",
            r.pick(data::STREET_KINDS),
            r.pick(data::STREETS),
            r.range(1, 180)
        ),
        DataLocale::Uk => format!(
            "{} {} {}",
            r.range(1, 240),
            r.pick(en::STREETS_UK),
            r.pick(en::STREET_KINDS_UK)
        ),
        DataLocale::Us => format!(
            "{} {} {}",
            r.range(100, 9899),
            r.pick(en::STREETS_US),
            r.pick(en::STREET_KINDS_US)
        ),
    }
}

impl Person {
    pub fn new(r: &mut Rng, locale: DataLocale) -> Self {
        let female = r.chance(0.5);
        let first = match (locale, female) {
            (DataLocale::It, true) => *r.pick(data::FEMALE),
            (DataLocale::It, false) => *r.pick(data::MALE),
            (_, true) => *r.pick(en::FEMALE),
            (_, false) => *r.pick(en::MALE),
        };
        let born = r.range(
            it::days_from_civil(1945, 1, 1),
            it::days_from_civil(2004, 12, 31),
        );
        Self {
            locale,
            first,
            last: r.pick(surnames(locale)),
            female,
            born,
            city: r.index(cities(locale)),
            second: r.pick(surnames(locale)),
        }
    }

    pub fn city(&self) -> (&'static str, &'static str) {
        match self.locale {
            DataLocale::It => {
                let c = data::CITIES[self.city];
                (c.0, c.1)
            }
            DataLocale::Uk => {
                let c = en::CITIES_UK[self.city];
                (c.0, c.1)
            }
            DataLocale::Us => {
                let c = en::CITIES_US[self.city];
                (c.0, c.1)
            }
        }
    }

    pub fn postcode(&self, r: &mut Rng) -> String {
        match self.locale {
            DataLocale::It => data::CITIES[self.city].2.to_owned(),
            DataLocale::Uk => {
                let (_, _, area, max) = en::CITIES_UK[self.city];
                format!(
                    "{area}{} {}{}{}",
                    r.range(1, i64::from(max)),
                    r.range(1, 9),
                    char::from(*r.pick(en::INWARD)),
                    char::from(*r.pick(en::INWARD))
                )
            }
            DataLocale::Us => format!("{}{}", en::CITIES_US[self.city].2, digits(r, 2)),
        }
    }

    pub fn phone(&self, r: &mut Rng) -> String {
        match self.locale {
            DataLocale::It => format!("+39 3{} {} {}", digits(r, 2), digits(r, 3), digits(r, 4)),
            DataLocale::Uk => format!("+44 7700 900{}", digits(r, 3)),
            DataLocale::Us => format!("+1 ({}) 555-01{}", en::CITIES_US[self.city].3, digits(r, 2)),
        }
    }

    pub fn email(&self, r: &mut Rng, attempt: u32) -> String {
        let n = if attempt > 0 || r.chance(0.35) {
            r.range(1, 99).to_string()
        } else {
            String::new()
        };
        let sep = *r.pick(&[".", ".", "_", ""]);
        format!(
            "{}{sep}{}{n}@{}",
            plain(self.first),
            plain(self.last),
            r.pick(data::DOMAINS)
        )
    }

    pub fn company(&self, r: &mut Rng) -> String {
        let (words, trades, forms) = match self.locale {
            DataLocale::It => (
                data::COMPANY_WORDS,
                data::COMPANY_TRADES,
                data::COMPANY_FORMS,
            ),
            DataLocale::Uk => (en::COMPANY_WORDS, en::COMPANY_TRADES, en::COMPANY_FORMS_UK),
            DataLocale::Us => (en::COMPANY_WORDS, en::COMPANY_TRADES, en::COMPANY_FORMS_US),
        };
        let form = r.pick(forms);
        match (r.below(3), self.locale) {
            (0, _) => format!("{} {} {form}", r.pick(words), r.pick(trades)),
            (1, _) => format!("{} & {} {form}", self.last, self.second),
            (_, DataLocale::It) => format!("{} {} {form}", r.pick(trades), self.last),
            _ => format!("{} {} {form}", self.last, r.pick(trades)),
        }
    }

    pub fn codice_fiscale(&self) -> String {
        it::codice_fiscale(&it::Person {
            first: self.first,
            last: self.last,
            female: self.female,
            born: self.born,
            place: data::CITIES[self.city].3,
        })
    }
}
