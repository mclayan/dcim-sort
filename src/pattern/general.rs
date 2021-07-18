use crate::pattern::PatternElement;
use crate::image::ImgInfo;
use chrono::{DateTime, Local, Datelike, Timelike};
use crate::main;

pub struct ScreenshotPattern {
    segment_name: String,
}

impl ScreenshotPattern {
    pub fn new(seg_name: String) -> Box<dyn PatternElement> {
        Box::new(ScreenshotPattern {
            segment_name: seg_name
        })
    }
}
impl PatternElement for ScreenshotPattern {

    fn is_optional(&self) -> bool {
        true
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        match info.metadata() {
            Some(m) => {
                if m.is_screenshot() {
                    Some(self.segment_name.clone())
                }
                else {
                    None
                }
            }
            None => None
        }
    }
}


pub enum DateTimePart {
    /// Year, formatted as 'YYYY'
    Year,
    /// Month, formatted as 'mm'
    Month,
    /// Day, formatted as 'DD'
    Day,
    /// Hour, formatted as 'hh' in 24-hour format
    Hour,
    /// Minute, formatted as 'mm'
    Minute,
    /// Second, formatted as 'ss'
    Second
}

/// Pattern to generate a segment based on a timestamp
/// associated with the file. Can be configured via
/// separators. Values are always expanded to fixed-
/// width strings and padded with '0'.
pub struct DateTimePattern {
    fs_timestamp_fallback: bool,
    separator: char,
    default: String,
    pattern: Vec<DateTimePart>
}
pub struct DateTimePatternBuilder {
    fs_timestamp_fallback: bool,
    separator: char,
    default: String,
    pattern: Vec<DateTimePart>
}

impl DateTimePattern {
    pub fn new() -> DateTimePatternBuilder {
        DateTimePatternBuilder {
            fs_timestamp_fallback: false,
            separator: '-',
            default: String::from("unknown"),
            pattern: Vec::new()
        }
    }
    fn generate_result(&self, ts: &DateTime<Local>) -> String {
        let mut result = String::new();
        let mut first = true;
        for part in &self.pattern {
            if first {
                first = false;
            }
            else {
                result.push(self.separator);
            }
            match part {
                DateTimePart::Year => result.push_str(format!("{:04}", ts.year()).as_str()),
                DateTimePart::Month => result.push_str(format!("{:02}", ts.month()).as_str()),
                DateTimePart::Day => result.push_str(format!("{:02}", ts.day()).as_str()),
                DateTimePart::Hour => result.push_str(format!("{:02}", ts.hour()).as_str()),
                DateTimePart::Minute => result.push_str(format!("{:02}", ts.minute()).as_str()),
                DateTimePart::Second => result.push_str(format!("{:02}", ts.second()).as_str()),
            }
        }
        result
    }
}
impl PatternElement for DateTimePattern {
    fn is_optional(&self) -> bool {
        false
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        let timestamp : Option<&DateTime<Local>> = match info.metadata() {
            Some(meta) => meta.created_at(),
            None => {
                if self.fs_timestamp_fallback {
                    Some(info.changed_at())
                }
                else {
                    None
                }
            }
        };
        let result = match timestamp {
            Some(ts) => self.generate_result(ts),
            None => self.default.clone()
        };
        Some(result)
    }
}
impl DateTimePatternBuilder {
    pub fn part(mut self, p: DateTimePart) -> DateTimePatternBuilder {
        self.pattern.push(p);
        self
    }

    pub fn separator(mut self, s: char) -> DateTimePatternBuilder {
        self.separator = s;
        self
    }

    pub fn default(mut self, s: String) -> DateTimePatternBuilder {
        self.default = s;
        self
    }

    pub fn fs_timestamp_fallback(mut self, b: bool) -> DateTimePatternBuilder {
        self.fs_timestamp_fallback = b;
        self
    }

    pub fn build(mut self) -> Box<dyn PatternElement> {
        if self.pattern.len() == 0 {
            self.pattern = vec![DateTimePart::Year, DateTimePart::Month]
        }
        Box::new(DateTimePattern{
            fs_timestamp_fallback: self.fs_timestamp_fallback,
            separator: self.separator,
            default: self.default,
            pattern: self.pattern
        })
    }
}