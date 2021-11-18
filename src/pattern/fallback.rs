use std::rc::Rc;
use crate::pattern::PatternElement;
use crate::media::ImgInfo;

enum GeneralFileType {
    Video,
    Picture,
    Audio,
    Text,
    Document,
    Other,
}

impl GeneralFileType {
    pub fn from(extension: &str) -> GeneralFileType {
        match extension.to_lowercase().as_str() {
            "mov" | "mp4" | "mpeg" | "mpg" | "ts"  | "mkv" |"avi" => GeneralFileType::Video,
            "mp3" | "wav" | "flac" | "ogg" | "wma" => GeneralFileType::Audio,
            "pdf" | "doc" | "docx" | "rtf" | "odt" => GeneralFileType::Document,
            "txt" | "ini" | "json" => GeneralFileType::Text,
            _ => GeneralFileType::Other
        }
    }
}

#[derive(Clone)]
pub struct SimpleFileTypePattern {
    video: String,
    picture: String,
    audio: String,
    text: String,
    document: String,
    other: String
}
pub struct SimpleFileTypePatternBuilder {
    video: String,
    picture: String,
    audio: String,
    text: String,
    document: String,
    other: String
}
impl SimpleFileTypePatternBuilder {
    pub fn video(mut self, s: String) -> SimpleFileTypePatternBuilder {
        self.video = s;
        self
    }

    pub fn picture(mut self, s: String) -> SimpleFileTypePatternBuilder {
        self.picture = s;
        self
    }

    pub fn text(mut self, s: String) -> SimpleFileTypePatternBuilder {
        self.text = s;
        self
    }

    pub fn audio(mut self, s: String) -> SimpleFileTypePatternBuilder {
        self.audio = s;
        self
    }

    pub fn document(mut self, s: String) -> SimpleFileTypePatternBuilder {
        self.document = s;
        self
    }

    pub fn other(mut self, s: String) -> SimpleFileTypePatternBuilder {
        self.other = s;
        self
    }

    pub fn build(mut self) -> Box<dyn PatternElement + Send> {
        Box::new(SimpleFileTypePattern{
            video: self.video,
            picture: self.picture,
            audio: self.audio,
            text: self.text,
            document: self.document,
            other: self.other
        })
    }
}
impl PatternElement for SimpleFileTypePattern {
    fn is_optional(&self) -> bool {
        true
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        if let Some(ex) = info.path().extension() {
            let extension = ex.to_str().unwrap_or("");
            let result = match GeneralFileType::from(extension) {
                GeneralFileType::Video => &self.video,
                GeneralFileType::Picture => &self.picture,
                GeneralFileType::Audio => &self.audio,
                GeneralFileType::Text => &self.text,
                GeneralFileType::Document => &self.document,
                GeneralFileType::Other => &self.other,
            };
            Some(result.clone())
        }
        else {
            Some(self.other.clone())
        }
    }

    fn display(&self) -> String {
        format!("video=\"{}\" pic=\"{}\" audio=\"{}\" txt=\"{}\" doc=\"{}\" other=\"{}\"",
            &self.video,
            &self.picture,
            &self.audio,
            &self.text,
            &self.document,
            &self.other
        )
    }

    fn name(&self) -> &str {
        "SimpleFileTypePattern"
    }

    fn clone_boxed(&self) -> Box<dyn PatternElement + Send> {
        Box::new(self.clone())
    }
}
impl SimpleFileTypePattern {
    pub fn def_video() -> String {
        String::from("videos")
    }

    pub fn def_picture() -> String {
        String::from("pictures")
    }

    pub fn def_audio() -> String {
        String::from("audio_files")
    }

    pub fn def_text() -> String {
        String::from("text_files")
    }

    pub fn def_document() -> String {
        String::from("documents")
    }

    pub fn def_other() -> String {
        String::from("other")
    }

    pub fn new() -> SimpleFileTypePatternBuilder {
        SimpleFileTypePatternBuilder {
            video: Self::def_video(),
            picture: Self::def_picture(),
            audio: Self::def_audio(),
            text: Self::def_text(),
            document: Self::def_document(),
            other: Self::def_other()
        }
    }
}