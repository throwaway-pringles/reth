//! Support for representing the version of the `eth`. [`Capability`].

use crate::capability::Capability;
use std::str::FromStr;
use thiserror::Error;

/// Error thrown when failed to parse a valid [`EthVersion`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Unknown eth protocol version: {0}")]
pub struct ParseVersionError(String);

/// The `eth` protocol version.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum EthVersion {
    /// The `eth` protocol version 66.
    Eth66 = 66,

    /// The `eth` protocol version 67.
    Eth67 = 67,

    /// The `eth` protocol version 68.
    Eth68 = 68,

    Istanbul65 = 65,
    Istanbul64 = 64,
}

/// The `eth` protocol version.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum IstanbulVersion {
    Istanbul65 = 65,
    Istanbul64 = 64,
}

impl EthVersion {
    /// The latest known eth version
    pub const LATEST: EthVersion = EthVersion::Eth68;

    /// Returns the total number of messages the protocol version supports.
    pub fn total_messages(&self) -> u8 {
        match self {
            EthVersion::Istanbul65 => 23, 
            EthVersion::Istanbul64 => 21,
            EthVersion::Eth66 => 15,
            EthVersion::Eth67 | EthVersion::Eth68 => {
                // eth/67,68 are eth/66 minus GetNodeData and NodeData messages
                13
            }
        }
    }
}

// FIXME: For full conversion, we need to separate IstanbulVersion with EthVersion in all places referring to EthVersion
impl IstanbulVersion {
    /// The latest known eth version
    pub const LATEST: IstanbulVersion = IstanbulVersion::Istanbul65;

    /// Returns the total number of messages the protocol version supports.
    pub fn total_messages(&self) -> u8 {
        match self {
            IstanbulVersion::Istanbul65 => 23,
            IstanbulVersion::Istanbul64 => 21,
        }
    }
}

/// Allow for converting from a `&str` to an `EthVersion`.
///
/// # Example
/// ```
/// use reth_eth_wire::types::EthVersion;
///
/// let version = EthVersion::try_from("67").unwrap();
/// assert_eq!(version, EthVersion::Eth67);
/// ```
impl TryFrom<&str> for EthVersion {
    type Error = ParseVersionError;

    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "64" => Ok(EthVersion::Istanbul64),
            "65" => Ok(EthVersion::Istanbul65),
            "66" => Ok(EthVersion::Eth66),
            "67" => Ok(EthVersion::Eth67),
            "68" => Ok(EthVersion::Eth68),
            _ => Err(ParseVersionError(s.to_string())),
        }
    }
}

impl TryFrom<&str> for IstanbulVersion {
    type Error = ParseVersionError;

    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "64" => Ok(IstanbulVersion::Istanbul64),
            "65" => Ok(IstanbulVersion::Istanbul65),
            _ => Err(ParseVersionError(s.to_string())),
        }
    }
}

/// Allow for converting from a u8 to an `EthVersion`.
///
/// # Example
/// ```
/// use reth_eth_wire::types::EthVersion;
///
/// let version = EthVersion::try_from(67).unwrap();
/// assert_eq!(version, EthVersion::Eth67);
/// ```
impl TryFrom<u8> for EthVersion {
    type Error = ParseVersionError;

    #[inline]
    fn try_from(u: u8) -> Result<Self, Self::Error> {
        match u {
            64 => Ok(EthVersion::Istanbul64),
            65 => Ok(EthVersion::Istanbul65),
            66 => Ok(EthVersion::Eth66),
            67 => Ok(EthVersion::Eth67),
            68 => Ok(EthVersion::Eth68),
            _ => Err(ParseVersionError(u.to_string())),
        }
    }
}

impl TryFrom<u8> for IstanbulVersion {
    type Error = ParseVersionError;

    #[inline]
    fn try_from(u: u8) -> Result<Self, Self::Error> {
        match u {
            64 => Ok(IstanbulVersion::Istanbul64),
            65 => Ok(IstanbulVersion::Istanbul65),
            _ => Err(ParseVersionError(u.to_string())),
        }
    }
}

impl FromStr for EthVersion {
    type Err = ParseVersionError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        EthVersion::try_from(s)
    }
}

impl FromStr for IstanbulVersion {
    type Err = ParseVersionError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        IstanbulVersion::try_from(s)
    }
}

impl From<EthVersion> for u8 {
    #[inline]
    fn from(v: EthVersion) -> u8 {
        v as u8
    }
}

impl From<IstanbulVersion> for u8 {
    #[inline]
    fn from(v: IstanbulVersion) -> u8 {
        v as u8
    }
}

impl From<EthVersion> for &'static str {
    #[inline]
    fn from(v: EthVersion) -> &'static str {
        match v {
            EthVersion::Istanbul64 => "64",
            EthVersion::Istanbul65 => "65",
            EthVersion::Eth66 => "66",
            EthVersion::Eth67 => "67",
            EthVersion::Eth68 => "68",
        }
    }
}


impl From<IstanbulVersion> for &'static str {
    #[inline]
    fn from(v: IstanbulVersion) -> &'static str {
        match v {
            IstanbulVersion::Istanbul65 => "65",
            IstanbulVersion::Istanbul64 => "64",
        }
    }
}

impl From<EthVersion> for Capability {
    #[inline]
    fn from(v: EthVersion) -> Capability {
        Capability { name: "eth".into(), version: v as usize }
    }
}

impl From<IstanbulVersion> for Capability {
    #[inline]
    fn from(v: IstanbulVersion) -> Capability {
        Capability { name: "istanbul".into(), version: v as usize }
    }
}

#[cfg(test)]
mod test {
    use super::{EthVersion, ParseVersionError};
    use std::{convert::TryFrom, string::ToString};

    #[test]
    fn test_eth_version_try_from_str() {
        assert_eq!(EthVersion::Eth66, EthVersion::try_from("66").unwrap());
        assert_eq!(EthVersion::Eth67, EthVersion::try_from("67").unwrap());
        assert_eq!(EthVersion::Eth68, EthVersion::try_from("68").unwrap());
        assert_eq!(Err(ParseVersionError("69".to_string())), EthVersion::try_from("69"));
    }

    #[test]
    fn test_eth_version_from_str() {
        assert_eq!(EthVersion::Eth66, "66".parse().unwrap());
        assert_eq!(EthVersion::Eth67, "67".parse().unwrap());
        assert_eq!(EthVersion::Eth68, "68".parse().unwrap());
        assert_eq!(Err(ParseVersionError("69".to_string())), "69".parse::<EthVersion>());
    }
}
