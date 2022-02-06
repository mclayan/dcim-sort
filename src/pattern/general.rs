use chrono::{Datelike, DateTime, Local, Timelike};
use regex::{Regex, RegexBuilder};

use crate::media::ImgInfo;
use crate::pattern::PatternElement;

static INVALID_REGEX_STR: &str = "the provided filename pattern is not a valid regex string";

/// A pattern that will translate to a static segment name in case a file was identified as a
/// screenshot. It evaluates the flag [crate::media::ImgMeta::is_screenshot] which was set by
/// metadata processing and optionally matches the filename against a RegEx. If either methods
/// indicate a screenshot, it translates to the static segment name or None if neither do.
pub struct ScreenshotPattern {
    segment_name: String,
    filename_pattern: Option<Regex>
}

impl ScreenshotPattern {
    pub fn def_value() -> String {
        String::from("screenshots")
    }

    /// Create a new pattern instance that identifies screenshots based on the flag
    /// [crate::media::ImgMeta::is_screenshot]
    pub fn new(seg_name: String) -> Box<dyn PatternElement + Send> {
        if seg_name.is_empty() {
            eprintln!("WARNING: screenshot pattern translates to an empty string!");
        }

        Box::new(ScreenshotPattern {
            segment_name: seg_name,
            filename_pattern: None
        })
    }

    /// Create a new pattern instance that tries to identify screenshots based on the filename
    /// instead of just the flag [crate::media::ImgMeta::is_screenshot]
    pub fn with_fname_matching(seg_name: String, filename_pattern: &str, case_insensitive: bool) -> Result<Box<dyn PatternElement + Send>, String> {
        if filename_pattern.is_empty() {
            return Err(INVALID_REGEX_STR.to_string());
        }
        let regex = match RegexBuilder::new(filename_pattern).case_insensitive(case_insensitive).build() {
            Ok(r) => r,
            Err(_e) => {
                return Err(INVALID_REGEX_STR.to_string());
            }
        };
        Ok(
            Box::new(ScreenshotPattern{
                segment_name: seg_name,
                filename_pattern: Some(regex)
            })
        )
    }

    /* === getters === */

    pub fn segment_name(&self) -> &str {
        self.segment_name.as_str()
    }

    pub fn filename_pattern(&self) -> Option<&Regex> {
        match &self.filename_pattern {
            Some(r) => Some(r),
            None => None
        }
    }
}
impl PatternElement for ScreenshotPattern {

    fn is_optional(&self) -> bool {
        true
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        let name_matches = match &self.filename_pattern {
            None => false,
            Some(regex) => match info.path().file_name() {
                Some(name) => match name.to_str() {
                    Some(n) => regex.is_match(n),
                    None => false
                },
                None => false
            }
        };

        let m = info.metadata();
        if m.is_screenshot() || name_matches {
            Some(self.segment_name.clone())
        }
        else {
            None
        }
    }

    fn display(&self) -> String {
        format!("name=\"{}\"", self.segment_name)
    }

    fn name(&self) -> &str {
        "ScreenshotPattern"
    }

    fn clone_boxed(&self) -> Box<dyn PatternElement + Send> {
        Box::new(ScreenshotPattern{
            segment_name: self.segment_name.clone(),
            filename_pattern: match &self.filename_pattern {
                None => None,
                Some(r) => Some(r.clone())
            }
        })
    }
}

#[derive(Clone)]
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

impl DateTimePart {
    pub fn parse(s: &str) -> Option<DateTimePart> {
        match s.to_lowercase().as_str() {
            "year"   => Some(DateTimePart::Year),
            "month"  => Some(DateTimePart::Month),
            "day"    => Some(DateTimePart::Day),
            "hour"   => Some(DateTimePart::Hour),
            "minute" => Some(DateTimePart::Minute),
            "second" => Some(DateTimePart::Second),
            _        => None
        }
    }
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
    pub fn def_fs_timestamp_fallback() -> bool {
        false
    }

    pub fn def_separator() -> char {
        '-'
    }

    pub fn def_default() -> String {
        String::from("unknown")
    }

    pub fn new() -> DateTimePatternBuilder {
        DateTimePatternBuilder {
            fs_timestamp_fallback: Self::def_fs_timestamp_fallback(),
            separator: Self::def_separator(),
            default: Self::def_default(),
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

    /* === getters === */

    pub fn fs_timestamp_fallback(&self) -> bool {
        self.fs_timestamp_fallback
    }

    pub fn separator(&self) -> char {
        self.separator
    }

    pub fn default(&self) -> &str {
        self.default.as_str()
    }

    pub fn pattern(&self) -> &[DateTimePart] {
        self.pattern.as_slice()
    }
}

impl PatternElement for DateTimePattern {
    fn is_optional(&self) -> bool {
        false
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        let timestamp : Option<&DateTime<Local>> = match info.metadata().created_at() {
            Some(ts) => Some(ts),
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

    fn display(&self) -> String {
        let mut s = String::new();
        let mut first = true;

        for p in &self.pattern {
            let ps = match p {
                DateTimePart::Year => 'y',
                DateTimePart::Month => 'M',
                DateTimePart::Day => 'd',
                DateTimePart::Hour => 'h',
                DateTimePart::Minute => 'm',
                DateTimePart::Second => 's'
            };
            if first {
                first = false;
            }
            else {
                s.push(self.separator);
            }
            s.push(ps);
        }
        format!("pattern=\"{}\" default=\"{}\" fs_ts_fallback=\"{}\"",
            s,
            &self.default,
            self.fs_timestamp_fallback
        )
    }

    fn name(&self) -> &str {
        "DateTimePattern"
    }

    fn clone_boxed(&self) -> Box<dyn PatternElement + Send> {
        Box::new(DateTimePattern{
            fs_timestamp_fallback: self.fs_timestamp_fallback,
            separator: self.separator,
            default: self.default.clone(),
            pattern: self.pattern.clone()
        })
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

    pub fn push_part(&mut self, part: DateTimePart) {
        self.pattern.push(part);
    }

    pub fn build(mut self) -> Box<dyn PatternElement + Send> {
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