// Copyright 2019, 2020 Wingchain
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::str::FromStr;

pub use custom_lib::{CDsa, CDsaConf, CKeyPair, CVerifier};

use crate::dsa::custom_lib::CustomLib;
use crate::dsa::ed25519::Ed25519;
use crate::dsa::sm2::SM2;
use crate::errors;
use std::path::PathBuf;

mod custom_lib;
mod ed25519;
mod sm2;

pub trait Dsa {
	type Error;
	type KeyPair: KeyPair;
	type Verifier: Verifier;

	fn name(&self) -> String;

	/// dylib impl demands that Self::KeyPair should not out live self
	fn generate_key_pair(&self) -> Result<Self::KeyPair, Self::Error>;

	/// dylib impl demands that Self::KeyPair should not out live self
	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> Result<Self::KeyPair, Self::Error>;

	/// dylib impl demands that Self::KeyPair should not out live self
	fn verifier_from_public_key(&self, public_key: &[u8]) -> Result<Self::Verifier, Self::Error>;
}

pub trait KeyPair {
	fn public_key(&self) -> Vec<u8>;
	fn secret_key(&self) -> Vec<u8>;
	fn sign(&self, message: &[u8]) -> Vec<u8>;
}

pub trait Verifier {
	type Error;
	fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Self::Error>;
}

pub enum DsaImpl {
	Ed25519,
	SM2,
	/// custom dsa impl provided by dylib
	Custom(CustomLib),
}

pub enum KeyPairImpl {
	Ed25519(<Ed25519 as Dsa>::KeyPair),
	SM2(<SM2 as Dsa>::KeyPair),
	Custom(<CustomLib as Dsa>::KeyPair),
}

pub enum VerifierImpl {
	Ed25519(<Ed25519 as Dsa>::Verifier),
	SM2(<SM2 as Dsa>::Verifier),
	Custom(<CustomLib as Dsa>::Verifier),
}

impl Dsa for DsaImpl {
	type Error = errors::Error;
	type KeyPair = KeyPairImpl;
	type Verifier = VerifierImpl;

	#[inline]
	fn name(&self) -> String {
		match self {
			Self::Ed25519 => Ed25519.name(),
			Self::SM2 => SM2.name(),
			Self::Custom(custom) => custom.name(),
		}
	}

	#[inline]
	fn generate_key_pair(&self) -> errors::Result<Self::KeyPair> {
		match self {
			Self::Ed25519 => Ok(KeyPairImpl::Ed25519(Ed25519.generate_key_pair()?)),
			Self::SM2 => Ok(KeyPairImpl::SM2(SM2.generate_key_pair()?)),
			Self::Custom(custom) => Ok(KeyPairImpl::Custom(custom.generate_key_pair()?)),
		}
	}

	#[inline]
	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> errors::Result<Self::KeyPair> {
		match self {
			Self::Ed25519 => Ok(KeyPairImpl::Ed25519(
				Ed25519.key_pair_from_secret_key(secret_key)?,
			)),
			Self::SM2 => Ok(KeyPairImpl::SM2(SM2.key_pair_from_secret_key(secret_key)?)),
			Self::Custom(custom) => Ok(KeyPairImpl::Custom(
				custom.key_pair_from_secret_key(secret_key)?,
			)),
		}
	}

	#[inline]
	fn verifier_from_public_key(&self, public_key: &[u8]) -> errors::Result<Self::Verifier> {
		match self {
			Self::Ed25519 => Ok(VerifierImpl::Ed25519(
				Ed25519.verifier_from_public_key(public_key)?,
			)),
			Self::SM2 => Ok(VerifierImpl::SM2(SM2.verifier_from_public_key(public_key)?)),
			Self::Custom(custom) => Ok(VerifierImpl::Custom(
				custom.verifier_from_public_key(public_key)?,
			)),
		}
	}
}

impl FromStr for DsaImpl {
	type Err = errors::Error;
	#[inline]
	fn from_str(s: &str) -> Result<DsaImpl, Self::Err> {
		match s {
			"ed25519" => Ok(DsaImpl::Ed25519),
			"sm2" => Ok(DsaImpl::SM2),
			other => {
				let path = PathBuf::from(&other);
				let custom_lib = CustomLib::new(&path)?;
				Ok(DsaImpl::Custom(custom_lib))
			}
		}
	}
}

impl KeyPair for KeyPairImpl {
	fn public_key(&self) -> Vec<u8> {
		match self {
			KeyPairImpl::Ed25519(kp) => kp.public_key(),
			KeyPairImpl::SM2(kp) => kp.public_key(),
			KeyPairImpl::Custom(kp) => kp.public_key(),
		}
	}
	fn secret_key(&self) -> Vec<u8> {
		match self {
			KeyPairImpl::Ed25519(kp) => kp.secret_key(),
			KeyPairImpl::SM2(kp) => kp.secret_key(),
			KeyPairImpl::Custom(kp) => kp.secret_key(),
		}
	}
	fn sign(&self, message: &[u8]) -> Vec<u8> {
		match self {
			KeyPairImpl::Ed25519(kp) => kp.sign(message),
			KeyPairImpl::SM2(kp) => kp.sign(message),
			KeyPairImpl::Custom(kp) => kp.sign(message),
		}
	}
}

impl Verifier for VerifierImpl {
	type Error = errors::Error;
	fn verify(&self, message: &[u8], signature: &[u8]) -> errors::Result<()> {
		match self {
			VerifierImpl::Ed25519(p) => p.verify(message, signature),
			VerifierImpl::SM2(p) => p.verify(message, signature),
			VerifierImpl::Custom(p) => p.verify(message, signature),
		}
	}
}
