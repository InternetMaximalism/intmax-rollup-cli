use std::{fmt::Display, str::FromStr};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = anyhow::Error;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        if !version.starts_with('v') {
            anyhow::bail!("Given version number is not prefixed with 'v'.");
        }

        let mut version = version
            .trim_start_matches('v')
            .split('.')
            .map(|v| v.parse().unwrap())
            .collect::<Vec<_>>();
        if version.is_empty() || version.len() > 3 {
            anyhow::bail!("Given version has invalid format.");
        }

        version.resize(3, 0);

        Ok(Self {
            major: version[0],
            minor: version[1],
            patch: version[2],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version_str = "v0.4.3";
        let version: Version = version_str.parse().unwrap();
        assert_eq!(version.to_string(), version_str);

        let version2_str = "v0.5";
        let version2: Version = version2_str.parse().unwrap();
        assert!(version < version2);

        let version3_str = "v1";
        let version3: Version = version3_str.parse().unwrap();
        assert!(version2 < version3);
    }
}
