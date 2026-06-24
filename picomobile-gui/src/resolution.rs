use {
    serde::{
        Deserialize,
        Deserializer,
        Serialize,
        Serializer,
        de,
    },
    std::{
        fmt,
        str::FromStr,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Resolution {
    R160x120,
    R320x240,
    #[default]
    R400x296,
    R640x480,
    R1024x768,
}

impl FromStr for Resolution {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "160x120" => Ok(Resolution::R160x120),
            "320x240" => Ok(Resolution::R320x240),
            "400x296" => Ok(Resolution::R400x296),
            "640x480" => Ok(Resolution::R640x480),
            "1024x768" => Ok(Resolution::R1024x768),
            _ => Err("Invalid resolution string"),
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        match self {
            Self::R160x120 => write!(f, "160x120"),
            Self::R320x240 => write!(f, "320x240"),
            Self::R400x296 => write!(f, "400x296"),
            Self::R640x480 => write!(f, "640x480"),
            Self::R1024x768 => write!(f, "1024x768"),
        }
    }
}

impl Serialize for Resolution {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for Resolution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}
