use std::fmt::{self, Write};
use std::io::Read;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use nom;
use penny::{Currency, Money};

use ast;
use ast::parse::Parsed;
use parse;

mod type_codes;
pub use self::type_codes::*;

// From std::fmt::builders (MIT/Apache-2.0)
struct PadAdapter<'a, 'b: 'a> {
    fmt: &'a mut fmt::Formatter<'b>,
    on_newline: bool,
    levels: u8,
}
impl<'a, 'b: 'a> PadAdapter<'a, 'b> {
    fn new_levels(fmt: &'a mut fmt::Formatter<'b>, levels: u8) -> PadAdapter<'a, 'b> {
        PadAdapter {
            fmt: fmt,
            on_newline: true,
            levels: levels,
        }
    }

    fn new(fmt: &'a mut fmt::Formatter<'b>) -> PadAdapter<'a, 'b> {
        PadAdapter::new_levels(fmt, 1)
    }
}
impl<'a, 'b: 'a> Write for PadAdapter<'a, 'b> {
    fn write_str(&mut self, mut s: &str) -> fmt::Result {
        while !s.is_empty() {
            if self.on_newline {
                for _ in 0..self.levels {
                    self.fmt.write_str("    ")?;
                }
            }
            let split = match s.find('\n') {
                Some(pos) => {
                    self.on_newline = true;
                    pos + 1
                }
                None => {
                    self.on_newline = false;
                    s.len()
                }
            };
            self.fmt.write_str(&s[..split])?;
            s = &s[split..];
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub enum BaiDateTime {
    DateTime(NaiveDateTime),
    DateEndOfDay(NaiveDate),
}
impl fmt::Display for BaiDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BaiDateTime::DateTime(ref dt) => write!(f, "{}", dt),
            BaiDateTime::DateEndOfDay(ref d) => write!(f, "{}Teod", d),
        }
    }
}
impl From<NaiveDateTime> for BaiDateTime {
    fn from(datetime: NaiveDateTime) -> Self {
        BaiDateTime::DateTime(datetime)
    }
}
impl From<NaiveDate> for BaiDateTime {
    fn from(date: NaiveDate) -> Self {
        BaiDateTime::DateEndOfDay(date)
    }
}

impl BaiDateTime {
    pub fn date(&self) -> NaiveDate {
        match *self {
            BaiDateTime::DateTime(dt) => dt.date(),
            BaiDateTime::DateEndOfDay(d) => d.clone(),
        }
    }
    pub fn time(&self) -> Option<NaiveTime> {
        match *self {
            BaiDateTime::DateTime(dt) => Some(dt.time()),
            BaiDateTime::DateEndOfDay(_d) => None,
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub enum BaiDateOrTime {
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    DateEndOfDay(NaiveDate),
}
impl fmt::Display for BaiDateOrTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BaiDateOrTime::Date(ref d) => write!(f, "{}", d),
            BaiDateOrTime::DateTime(ref dt) => write!(f, "{}", dt),
            BaiDateOrTime::DateEndOfDay(ref d) => write!(f, "{}Teod", d),
        }
    }
}
impl From<NaiveDate> for BaiDateOrTime {
    fn from(date: NaiveDate) -> Self {
        BaiDateOrTime::Date(date)
    }
}
impl From<BaiDateTime> for BaiDateOrTime {
    fn from(datetime: BaiDateTime) -> Self {
        match datetime {
            BaiDateTime::DateTime(dt) => BaiDateOrTime::DateTime(dt),
            BaiDateTime::DateEndOfDay(d) => BaiDateOrTime::DateEndOfDay(d),
        }
    }
}

impl BaiDateOrTime {
    pub fn date(self) -> NaiveDate {
        match self {
            BaiDateOrTime::Date(d) => d,
            BaiDateOrTime::DateTime(dt) => dt.date(),
            BaiDateOrTime::DateEndOfDay(d) => d,
        }
    }
    pub fn date_time(self) -> Option<BaiDateTime> {
        match self {
            BaiDateOrTime::Date(_) => None,
            BaiDateOrTime::DateTime(dt) => Some(dt.into()),
            BaiDateOrTime::DateEndOfDay(d) => Some(d.into()),
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct File {
    pub sender: Party,
    pub receiver: Party,
    pub creation: BaiDateTime,
    pub ident: FileIdent,
    pub groups: Vec<Group>,
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "File: {sender} to {receiver} at {creation} ({ident}) {{\n",
            sender = self.sender,
            receiver = self.receiver,
            creation = self.creation,
            ident = self.ident
        ).and_then(|()| {
            let mut f = PadAdapter::new(f);
            for group in &self.groups {
                write!(f, "{},\n", group)?
            };
            Ok(())
        }).and_then(|()| write!(f, "}}"))
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct Party(pub String);
impl fmt::Display for Party {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct FileIdent(pub u32);
impl fmt::Display for FileIdent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct Group {
    pub ultimate_receiver: Option<Party>,
    // Optional because banks seem to treat it as such ( :( )
    pub originator: Option<Party>,
    pub status: GroupStatus,
    pub as_of: BaiDateOrTime,
    pub currency: Option<Currency>,
    pub as_of_date_mod: Option<AsOfDateModifier>,
    pub accounts: Vec<Account>,
}

impl Group {
    pub fn currency_def(&self) -> Currency {
        self.currency.unwrap_or(Currency::USD)
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Group {}: ", self.status).and_then(|()| {
            match self.originator {
                Some(ref originator) => write!(f, "{}", originator),
                None => write!(f, "Unknown originator"),
            }
        }).and_then(|()| {
            match self.ultimate_receiver {
                None => Ok(()),
                Some(ref ultimate_receiver) => write!(f, " to {}", ultimate_receiver),
            }
        }).and_then(|()| {
            write!(f, " at {}", self.as_of)
        }).and_then(|()| {
            match self.as_of_date_mod {
                None => Ok(()),
                Some(ref as_of_date_mod) => write!(f, " ({})", as_of_date_mod),
            }
        }).and_then(|()| {
            write!(f, " in {}", self.currency_def())
        }).and_then(|()| {
            write!(f, " {{\n")
        }).and_then(|()| {
            let mut f = PadAdapter::new(f);
            for account in &self.accounts {
                write!(f, "{},\n", account)?;
            }
            Ok(())
        }).and_then(|()| write!(f, "}}"))
    }
}

enum_mapping! {
    #[derive(Debug, Clone, Copy)]
    #[cfg_attr(feature="serde-serialize", derive(Serialize, Deserialize))]
    pub GroupStatus(u8) {
        Update(1),
        Deletion(2),
        Correction(3),
        TestOnly(4),
    }
}
impl fmt::Display for GroupStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::GroupStatus as GS;
        match *self {
            GS::Update => write!(f, "Update"),
            GS::Deletion => write!(f, "Deletion"),
            GS::Correction => write!(f, "Correction"),
            GS::TestOnly => write!(f, "Test Only"),
        }
    }
}

enum_mapping! {
    #[derive(Debug, Clone, Copy)]
    #[cfg_attr(feature="serde-serialize", derive(Serialize, Deserialize))]
    pub AsOfDateModifier(u8) {
        InterimPrevious(1),
        FinalPrevious(2),
        InterimSame(3),
        FinalSame(4),
    }
}
impl fmt::Display for AsOfDateModifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::AsOfDateModifier as AODM;
        match *self {
            AODM::InterimPrevious => write!(f, "Interim previous-day data"),
            AODM::FinalPrevious => write!(f, "Final previous-day data"),
            AODM::InterimSame => write!(f, "Interim same-day data"),
            AODM::FinalSame => write!(f, "Final same-day data"),
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct Account {
    pub customer_account: AccountNumber,
    pub currency: Option<Currency>,
    pub infos: Vec<AccountInfo>,
    pub transaction_details: Vec<TransactionDetail>,
}

impl Account {
    pub fn currency_def(&self, group_cur: Currency) -> Currency {
        self.currency.unwrap_or(group_cur)
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Account {} ", self.customer_account).and_then(|()| {
            match self.currency {
                None => Ok(()),
                Some(c) => write!(f, "({}) ", c),
            }
        }).and_then(|()| {
            write!(f, "{{\n    Infos: [\n")
        }).and_then(|()| {
            let mut f = PadAdapter::new_levels(f, 2);
            for info in &self.infos {
                write!(f, "{},\n", info)?;
            }
            Ok(())
        }).and_then(|()| {
            write!(f, "    ],\n    Transaction Details: [\n")
        }).and_then(|()| {
            let mut f = PadAdapter::new_levels(f, 2);
            for details in &self.transaction_details {
                write!(f, "{},\n", details)?;
            }
            Ok(())
        }).and_then(|()| {
            write!(f, "    ],\n}}")
        })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub enum AccountInfo {
    Summary {
        code: SummaryCode,
        amount: Option<u64>,
        item_count: Option<u32>,
        funds: Option<FundsType>,
    },
    Status {
        code: StatusCode,
        amount: Option<i64>,
        funds: Option<FundsType>,
    },
}

impl AccountInfo {
    pub fn amount_money(&self, account_cur: Currency) -> Option<Money> {
        use self::AccountInfo as AI;
        match *self {
            AI::Summary {
                amount: Some(amount),
                ..
            } => Some(Money::new(amount as i64, account_cur)),
            AI::Status {
                amount: Some(amount),
                ..
            } => Some(Money::new(amount, account_cur)),
            _ => None,
        }
    }
}

impl fmt::Display for AccountInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AccountInfo::Status {
                code,
                amount,
                ref funds,
            } => match amount {
                None => write!(f, "{}", code),
                Some(amount) => write!(f, "{}: {}", code, amount),
            },
            AccountInfo::Summary {
                code,
                amount,
                item_count,
                ref funds,
            } => {
                match amount {
                    None => write!(f, "{} {{\n", code),
                    Some(amount) => write!(f, "{}: {} {{\n", code, amount),
                }?;
                {
                    let mut f = PadAdapter::new(f);
                    if let Some(item_count) = item_count {
                        write!(f, "Item count: {},\n", item_count)?
                    }
                    if let Some(funds) = funds.as_ref() {
                        write!(f, "Funds: {},\n", funds)?
                    }
                }
                write!(f, "}}")
            }
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct AccountNumber(pub String);
impl fmt::Display for AccountNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a#{:?}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub enum FundsType {
    Unknown, // Z (default)
    ImmediateAvail, // 0
    OneDayAvail, // 1
    TwoOrMoreDaysAvail, // 2
    DistributedAvailS {
        // S
        // These are optional because the example given treats them as such.
        immediate: Option<i64>,
        one_day: Option<i64>,
        more_than_one_day: Option<i64>,
    },
    ValueDated(BaiDateOrTime), // V
    DistributedAvailD(Vec<DistributedAvailDistribution>), // D
}
impl fmt::Display for FundsType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FundsType::Unknown => write!(f, "Funds"),
            FundsType::ImmediateAvail => write!(f, "Funds(Immediate)"),
            FundsType::OneDayAvail => write!(f, "Funds(One day)"),
            FundsType::TwoOrMoreDaysAvail => write!(f, "Funds(Two+ days)"),
            FundsType::DistributedAvailS {
                ref immediate,
                ref one_day,
                ref more_than_one_day,
            } => {
                write!(f, "Funds(Distributed avail) {{\n")?;
                {
                    let mut f = PadAdapter::new(f);
                    if let Some(immediate) = immediate.as_ref() {
                        write!(f, "Immediate avail: {},\n", immediate)?
                    }
                    if let Some(one_day) = one_day.as_ref() {
                        write!(f, "One-day avail: {},\n", one_day)?
                    }
                    if let Some(more_than_one_day) = more_than_one_day.as_ref() {
                        write!(f, "Two or more days avail: {},\n", more_than_one_day)?
                    }
                }
                write!(f, "}}")
            }
            FundsType::ValueDated(ref avail_date_or_time) => {
                write!(f, "Funds(Value dated): {}", avail_date_or_time)
            }
            FundsType::DistributedAvailD(ref dists) => {
                write!(f, "Funds(Distributed avail) [\n")?;
                {
                    let mut f = PadAdapter::new(f);
                    for dist in dists {
                        write!(f, "{} days: {},\n", dist.days, dist.amount)?;
                    }
                }
                write!(f, "]")
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct DistributedAvailDistribution {
    pub days: u32,
    pub amount: i64,
}

impl DistributedAvailDistribution {
    pub fn amount_money(&self, funds_cur: Currency) -> Money {
        Money::new(self.amount, funds_cur)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct TransactionDetail {
    pub code: DetailCode,
    pub amount: Option<i64>,
    pub funds: Option<FundsType>,
    pub bank_ref_num: Option<ReferenceNum>,
    pub customer_ref_num: Option<ReferenceNum>,
    pub text: Option<Vec<String>>,
}

impl TransactionDetail {
    pub fn amount_money(&self, account_cur: Currency) -> Option<Money> {
        self.amount.map(|amount| Money::new(amount, account_cur))
    }
}
impl fmt::Display for TransactionDetail {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Transaction: {}", self.code)?;
        if let Some(amount) = self.amount {
            write!(f, ": {}", amount)?;
        }
        write!(f, " {{\n")?;
        {
            let mut f = PadAdapter::new(f);
            if let Some(funds) = self.funds.as_ref() {
                write!(f, "Funds: {},\n", funds)?;
            }
            if let Some(bank_ref_num) = self.bank_ref_num.as_ref() {
                write!(f, "Bank: {},\n", bank_ref_num)?;
            }
            if let Some(customer_ref_num) = self.customer_ref_num.as_ref() {
                write!(f, "Customer: {},\n", customer_ref_num)?;
            }
            if let Some(text) = self.text.as_ref() {
                write!(f, "Text: {:#?},\n", text)?;
            }
        }
        write!(f, "}}")
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct ReferenceNum(pub String);
impl fmt::Display for ReferenceNum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "r#{:?}", self.0)
    }
}

#[derive(Debug, Clone)]
pub enum FileProcessError<'a> {
    Parse(nom::ErrorKind),
    FieldParse(ast::parse::ParseError<ast::Record<'a>>),
    UnfinishedConversion,
    Conversion(ast::convert::ConvertError),
}

impl File {
    pub fn process<'a>(file: &'a [u8]) -> Result<File, FileProcessError<'a>> {
        parse::file(file)
            .to_result()
            .map_err(FileProcessError::Parse)
            .and_then(|raw_records| {
                let mut parsed_records = raw_records.iter().map(|r| {
                    ast::Record::parse(r).map_err(FileProcessError::FieldParse)
                });
                ast::convert::Converter::fold_results(&mut parsed_records, |e| match e {
                    Some(e) => FileProcessError::Conversion(e),
                    None => FileProcessError::UnfinishedConversion,
                })
            })
    }

    pub fn from_source<T: Read>(source: &mut T) -> Result<File, String> {
        let mut file = Vec::new();
        source
            .read_to_end(&mut file)
            .map_err(|e| format!("{:?}", e))?;
        File::process(&file).map_err(|e| format!("{:?}", e))
    }
}
