use std::{fmt, str::FromStr};

use encoding_rs::{
    BIG5, EUC_KR, Encoding, GBK, SHIFT_JIS, WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252,
    WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyEncoding {
    Windows1250,
    Windows1251,
    Windows1252,
    Windows1253,
    Windows1254,
    Windows1255,
    Windows1256,
    Windows1257,
    Windows1258,
    Windows874,
    ShiftJis,
    EucKr,
    Gbk,
    Big5,
}

impl LegacyEncoding {
    pub fn label(self) -> &'static str {
        self.codec().name()
    }

    fn codec(self) -> &'static Encoding {
        match self {
            Self::Windows1250 => WINDOWS_1250,
            Self::Windows1251 => WINDOWS_1251,
            Self::Windows1252 => WINDOWS_1252,
            Self::Windows1253 => WINDOWS_1253,
            Self::Windows1254 => WINDOWS_1254,
            Self::Windows1255 => WINDOWS_1255,
            Self::Windows1256 => WINDOWS_1256,
            Self::Windows1257 => WINDOWS_1257,
            Self::Windows1258 => WINDOWS_1258,
            Self::Windows874 => WINDOWS_874,
            Self::ShiftJis => SHIFT_JIS,
            Self::EucKr => EUC_KR,
            Self::Gbk => GBK,
            Self::Big5 => BIG5,
        }
    }
}

impl fmt::Display for LegacyEncoding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl FromStr for LegacyEncoding {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "windows-1250" | "cp1250" => Ok(Self::Windows1250),
            "windows-1251" | "cp1251" => Ok(Self::Windows1251),
            "windows-1252" | "cp1252" => Ok(Self::Windows1252),
            "windows-1253" | "cp1253" => Ok(Self::Windows1253),
            "windows-1254" | "cp1254" => Ok(Self::Windows1254),
            "windows-1255" | "cp1255" => Ok(Self::Windows1255),
            "windows-1256" | "cp1256" => Ok(Self::Windows1256),
            "windows-1257" | "cp1257" => Ok(Self::Windows1257),
            "windows-1258" | "cp1258" => Ok(Self::Windows1258),
            "windows-874" | "cp874" => Ok(Self::Windows874),
            "shift-jis" | "shiftjis" | "sjis" => Ok(Self::ShiftJis),
            "euc-kr" | "euckr" => Ok(Self::EucKr),
            "gbk" | "gb2312" => Ok(Self::Gbk),
            "big5" => Ok(Self::Big5),
            _ => Err(format!(
                "unsupported legacy encoding '{value}'; use a Windows-125x code page, Windows-874, Shift-JIS, EUC-KR, GBK or Big5"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EncodingDecision {
    FontCharset,
    DefaultAnsiFallback,
    ExplicitOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodedLegacyString {
    pub text: String,
    pub encoding: LegacyEncoding,
    pub decision: EncodingDecision,
    pub had_errors: bool,
}

/// Map Windows `TFontCharset` values used by the legacy VCL application to an
/// explicit decoder. Symbol and OEM charsets deliberately return `None` because
/// they cannot be safely normalized as ordinary text without additional font or
/// system-codepage context.
pub fn encoding_from_font_charset(charset: u8) -> Option<LegacyEncoding> {
    match charset {
        0 => Some(LegacyEncoding::Windows1252), // ANSI_CHARSET
        1 => None,                              // DEFAULT_CHARSET: use explicit fallback
        2 => None,                              // SYMBOL_CHARSET
        128 => Some(LegacyEncoding::ShiftJis),
        129 => Some(LegacyEncoding::EucKr),
        134 => Some(LegacyEncoding::Gbk),
        136 => Some(LegacyEncoding::Big5),
        161 => Some(LegacyEncoding::Windows1253),
        162 => Some(LegacyEncoding::Windows1254),
        163 => Some(LegacyEncoding::Windows1258),
        177 => Some(LegacyEncoding::Windows1255),
        178 => Some(LegacyEncoding::Windows1256),
        186 => Some(LegacyEncoding::Windows1257),
        204 => Some(LegacyEncoding::Windows1251),
        222 => Some(LegacyEncoding::Windows874),
        238 => Some(LegacyEncoding::Windows1250),
        255 => None, // OEM_CHARSET is system dependent
        _ => None,
    }
}

pub fn decode_with_encoding(
    raw: &[u8],
    encoding: LegacyEncoding,
    decision: EncodingDecision,
) -> DecodedLegacyString {
    let (text, _, had_errors) = encoding.codec().decode(raw);
    DecodedLegacyString {
        text: text.into_owned(),
        encoding,
        decision,
        had_errors,
    }
}

/// Decode ordinary legacy ANSI strings using the stored/default font charset
/// when it identifies a concrete Windows code page. `DEFAULT_CHARSET` and
/// unknown charsets use the caller-supplied fallback explicitly rather than
/// silently assuming UTF-8.
pub fn decode_ansi_string(
    raw: &[u8],
    font_charset: u8,
    fallback: LegacyEncoding,
) -> DecodedLegacyString {
    if let Some(encoding) = encoding_from_font_charset(font_charset) {
        decode_with_encoding(raw, encoding, EncodingDecision::FontCharset)
    } else {
        decode_with_encoding(raw, fallback, EncodingDecision::DefaultAnsiFallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_windows_1252_umlauts_and_euro() {
        let decoded = decode_with_encoding(
            &[b'F', b'l', 0xfc, 0xdf, b'b', b'i', b'l', b'd', b' ', 0x80],
            LegacyEncoding::Windows1252,
            EncodingDecision::ExplicitOverride,
        );
        assert_eq!(decoded.text, "Flüßbild €");
        assert!(!decoded.had_errors);
    }

    #[test]
    fn maps_common_vcl_font_charsets() {
        assert_eq!(
            encoding_from_font_charset(0),
            Some(LegacyEncoding::Windows1252)
        );
        assert_eq!(
            encoding_from_font_charset(128),
            Some(LegacyEncoding::ShiftJis)
        );
        assert_eq!(
            encoding_from_font_charset(204),
            Some(LegacyEncoding::Windows1251)
        );
        assert_eq!(
            encoding_from_font_charset(238),
            Some(LegacyEncoding::Windows1250)
        );
        assert_eq!(encoding_from_font_charset(2), None);
        assert_eq!(encoding_from_font_charset(255), None);
    }

    #[test]
    fn default_charset_uses_explicit_fallback() {
        let decoded = decode_ansi_string(&[0xe4], 1, LegacyEncoding::Windows1252);
        assert_eq!(decoded.text, "ä");
        assert_eq!(decoded.decision, EncodingDecision::DefaultAnsiFallback);
    }

    #[test]
    fn parses_cli_encoding_aliases() {
        assert_eq!(
            "cp1252".parse::<LegacyEncoding>().unwrap(),
            LegacyEncoding::Windows1252
        );
        assert_eq!(
            "shift_jis".parse::<LegacyEncoding>().unwrap(),
            LegacyEncoding::ShiftJis
        );
        assert!("utf-8".parse::<LegacyEncoding>().is_err());
    }
}
