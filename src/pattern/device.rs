use crate::pattern::{PatternInitError, PatternElement};
use std::borrow::Borrow;
use crate::media::ImgInfo;

pub enum DevicePart {
    Make,
    Model
}

pub enum CaseNormalization {
    Lowercase,
    Uppercase,
    None,
}

pub struct MakeModelPattern {
    pattern: Vec<DevicePart>,
    separator: char,
    case: CaseNormalization,
    replace_spaces: bool,
    default_make: String,
    default_model: String
}

impl MakeModelPattern {
    pub fn new() -> MakeModelPatternBuilder {
        MakeModelPatternBuilder {
            pattern: Vec::<DevicePart>::new(),
            separator: '_',
            case: CaseNormalization::Lowercase,
            replace_spaces: true,
            default_make: String::from("unknown"),
            default_model: String::from("unknown")
        }
    }

    fn normalize_case(&self, s: String) -> String {
        let mut result = match self.case {
            CaseNormalization::Lowercase => s.to_lowercase(),
            CaseNormalization::Uppercase => s.to_uppercase(),
            CaseNormalization::None => s
        };
        result
    }
}
impl PatternElement for MakeModelPattern {
    fn is_optional(&self) -> bool {
        false
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        let (mut model, mut make) : (String, String);
        let meta = info.metadata();
        make = match meta.make() {
            "" => self.default_make.clone(),
            _s => String::from(_s)
        };
        model = match meta.model() {
            "" => self.default_model.clone(),
            _s => String::from(_s)
        };
        if self.replace_spaces {
            make = make.replace(' ', "-");
            model = model.replace(' ', "-");
        }
        else {
            make = self.default_make.clone();
            model = self.default_model.clone();
        }

        let mut result = String::new();
        let mut first = true;
        for p in &self.pattern {
            if first {
                first = false;
            }
            else {
                result.push(self.separator);
            }
            match p {
                DevicePart::Model => result.push_str(&model),
                DevicePart::Make => result.push_str(&make)
            }
        }
        Some(result)
    }
}

pub struct MakeModelPatternBuilder {
    pattern: Vec<DevicePart>,
    separator: char,
    case: CaseNormalization,
    replace_spaces: bool,
    default_make: String,
    default_model: String
}
impl MakeModelPatternBuilder {
    pub fn part(mut self, s: DevicePart) -> MakeModelPatternBuilder {
        self.pattern.push(s);
        self
    }

    pub fn case_normalization(mut self, c: CaseNormalization) -> MakeModelPatternBuilder {
        self.case = c;
        self
    }

    pub fn replace_spaces(mut self, b: bool) -> MakeModelPatternBuilder {
        self.replace_spaces = b;
        self
    }


    pub fn default_make(mut self, name: String) -> MakeModelPatternBuilder {
        self.default_make = name;
        self
    }

    pub fn default_model(mut self, name: String) -> MakeModelPatternBuilder {
        self.default_model = name;
        self
    }

    pub fn separator(mut self, separator: char) -> MakeModelPatternBuilder {
        self.separator = separator;
        self
    }

    pub fn build(mut self) -> Box<dyn PatternElement> {
        if self.pattern.len() < 1 {
            self.pattern.push(DevicePart::Make);
            self.pattern.push(DevicePart::Model);
        }
        Box::new(MakeModelPattern {
            pattern: self.pattern,
            separator: self.separator,
            case: self.case,
            replace_spaces: self.replace_spaces,
            default_make: self.default_make,
            default_model: self.default_model
        })
    }
}


