use crate::media::ImgInfo;
use crate::pattern::PatternElement;

#[derive(Clone)]
pub enum DevicePart {
    Make,
    Model
}

impl DevicePart {
    pub fn parse(s: &str) -> Option<DevicePart>{
        match s.to_lowercase().as_str() {
            "make" => Some(DevicePart::Make),
            "model" => Some(DevicePart::Model),
            _ => None
        }
    }
}

#[derive(Copy, Clone)]
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
    fallback: String,
    default_make: String,
    default_model: String
}

impl MakeModelPattern {
    pub fn def_replace_spaces() -> bool {
        true
    }

    pub fn def_separator() -> char {
        '_'
    }

    pub fn def_case() -> CaseNormalization {
        CaseNormalization::Lowercase
    }

    pub fn def_default_make() -> String {
        String::from("unknown")
    }

    pub fn def_default_model() -> String {
        String::from("unknown")
    }

    pub fn new() -> MakeModelPatternBuilder {
        MakeModelPatternBuilder {
            pattern: Vec::<DevicePart>::new(),
            separator: Self::def_separator(),
            case: Self::def_case(),
            replace_spaces: Self::def_replace_spaces(),
            fallback: String::new(),
            default_make: Self::def_default_make(),
            default_model: Self::def_default_model()
        }
    }

    fn normalize_case(&self, s: String) -> String {
        let result = match self.case {
            CaseNormalization::Lowercase => s.to_lowercase(),
            CaseNormalization::Uppercase => s.to_uppercase(),
            CaseNormalization::None => s
        };
        result
    }

    /* ==== getters ==== */

    pub fn pattern(&self) -> &[DevicePart] {
        self.pattern.as_slice()
    }

    pub fn separator(&self) -> char {
        self.separator
    }

    pub fn case_normalization(&self) -> &CaseNormalization {
        &self.case
    }

    pub fn replace_spaces(&self) -> bool {
        self.replace_spaces
    }

    pub fn fallback_value(&self) -> &str {
        self.fallback.as_str()
    }

    pub fn default_make(&self) -> &str {
        self.default_make.as_str()
    }

    pub fn default_model(&self) -> &str {
        self.default_model.as_str()
    }
}

impl PatternElement for MakeModelPattern {
    fn is_optional(&self) -> bool {
        false
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        let (mut model, mut make) : (String, String);
        let meta = info.metadata();
        make = self.normalize_case(match meta.make() {
            "" => self.default_make.clone(),
            _s => String::from(_s)
        });
        model = self.normalize_case(match meta.model() {
            "" => self.default_model.clone(),
            _s => String::from(_s)
        });
        if self.replace_spaces {
            make = make.replace(' ', "-");
            model = model.replace(' ', "-");
        }
        else {
            make = self.default_make.clone();
            model = self.default_model.clone();
        }

        let mut result = String::new();
        if make == self.default_make && model == self.default_model && !self.fallback.is_empty() {
            result = self.fallback.clone();
        }
        else {
            let mut first = true;
            for p in &self.pattern {
                if first {
                    first = false;
                } else {
                    result.push(self.separator);
                }
                match p {
                    DevicePart::Model => result.push_str(&model),
                    DevicePart::Make => result.push_str(&make)
                }
            }
        }
        Some(result)
    }

    fn display(&self) -> String {
        let mut pattern = String::new();
        let mut first = true;
        let case = match self.case {
            CaseNormalization::Lowercase => "lower",
            CaseNormalization::Uppercase => "upper",
            CaseNormalization::None => ""
        };

        for p in &self.pattern {
            let ps = match p {
                DevicePart::Make => "[MAKE]",
                DevicePart::Model => "[MODEL]"
            };
            if first {
                first = false;
            }
            else {
                pattern.push(self.separator);
            }
            pattern.push_str(ps);
        }
        format!("replace_spaces=\"{}\" case_norm=\"{}\" pattern=\"{}\" fallback=\"{}\" def_make=\"{} def_model=\"{}\"",
            self.replace_spaces,
            case,
            pattern,
            self.fallback,
            &self.default_make,
            &self.default_model
        )
    }

    fn name(&self) -> &str {
        "MakeModelPattern"
    }

    fn clone_boxed(&self) -> Box<dyn PatternElement + Send> {
        Box::new(MakeModelPattern{
            pattern: self.pattern.clone(),
            separator: self.separator,
            case: self.case.clone(),
            replace_spaces: self.replace_spaces,
            fallback: self.fallback.clone(),
            default_make: self.default_make.clone(),
            default_model: self.default_model.clone()
        })
    }
}

pub struct MakeModelPatternBuilder {
    pattern: Vec<DevicePart>,
    separator: char,
    case: CaseNormalization,
    replace_spaces: bool,
    fallback: String,
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

    pub fn fallback(mut self, fallback: String) -> MakeModelPatternBuilder {
        self.fallback = fallback;
        self
    }

    pub fn push_part(&mut self, part: DevicePart) {
        self.pattern.push(part);
    }

    pub fn build(mut self) -> Box<dyn PatternElement + Send> {
        Box::new(self.build_unboxed())
    }

    pub fn build_unboxed(mut self) -> MakeModelPattern {
        if self.pattern.len() < 1 {
            self.pattern.push(DevicePart::Make);
            self.pattern.push(DevicePart::Model);
        }
        MakeModelPattern {
            pattern: self.pattern,
            separator: self.separator,
            case: self.case,
            replace_spaces: self.replace_spaces,
            fallback: self.fallback,
            default_make: self.default_make,
            default_model: self.default_model
        }
    }
}


