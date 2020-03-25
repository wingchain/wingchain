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

#![feature(test)]

extern crate test;

use crypto::dsa::DsaImpl::SM2;
use crypto::dsa::{Dsa, DsaImpl, KeyPair, Verifier, CDsa, CKeyPair, CVerifier};
use crypto_dylib_samples_dsa::Ed25519;
use std::hint::black_box;
use test::Bencher;

#[bench]
fn bench_ed25519_sign_ring(b: &mut Bencher) {
	let dsa = DsaImpl::Ed25519;

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	b.iter(|| black_box(key_pair.sign(&message)));
}

#[bench]
fn bench_ed25519_sign_dalek(b: &mut Bencher) {
	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = Ed25519.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	let mut out = vec![0u8; 64];

	b.iter(|| black_box(key_pair.sign(&message, &mut out)));
}

#[bench]
fn bench_sm2_sign(b: &mut Bencher) {
	let dsa = DsaImpl::SM2;

	let secret: [u8; 32] = [
		128, 166, 19, 115, 227, 79, 114, 21, 254, 206, 184, 221, 6, 187, 55, 49, 234, 54, 47, 245,
		53, 90, 114, 38, 212, 225, 45, 7, 106, 126, 181, 136,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	b.iter(|| black_box(key_pair.sign(&message)));
}

#[bench]
fn bench_ed25519_sign_custom(b: &mut Bencher) {
	use std::str::FromStr;

	let path = utils::get_dylib("crypto_dylib_samples_dsa");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let dsa = DsaImpl::from_str(&path).unwrap();

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	b.iter(|| black_box(key_pair.sign(&message)));
}

#[bench]
fn bench_ed25519_sign_include_init_ring(b: &mut Bencher) {
	let dsa = DsaImpl::Ed25519;

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let message = [1u8; 256];

	let run = || {
		let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();
		key_pair.sign(&message)
	};

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_sign_include_init_dalek(b: &mut Bencher) {
	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let message = [1u8; 256];

	let run = || {
		let mut out = vec![0u8; 64];
		let key_pair = Ed25519.key_pair_from_secret_key(&secret).unwrap();
		key_pair.sign(&message, &mut out);
	};

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_sm2_sign_include_init(b: &mut Bencher) {
	let secret: [u8; 32] = [
		128, 166, 19, 115, 227, 79, 114, 21, 254, 206, 184, 221, 6, 187, 55, 49, 234, 54, 47, 245,
		53, 90, 114, 38, 212, 225, 45, 7, 106, 126, 181, 136,
	];

	let message = [1u8; 256];

	let run = || {
		let key_pair = SM2.key_pair_from_secret_key(&secret).unwrap();
		key_pair.sign(&message)
	};

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_sign_include_init_custom(b: &mut Bencher) {
	use std::str::FromStr;

	let path = utils::get_dylib("crypto_dylib_samples_dsa");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();


	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let message = [1u8; 256];

	let dsa = DsaImpl::from_str(&path).unwrap();

	let run = || {
		let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();
		key_pair.sign(&message)
	};

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_verify_ring(b: &mut Bencher) {
	let dsa = DsaImpl::Ed25519;

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	let signature = key_pair.sign(&message);

	let verifier = dsa
		.verifier_from_public_key(&key_pair.public_key())
		.unwrap();

	verifier.verify(&message, &signature).unwrap();

	let run = || verifier.verify(&message, &signature);

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_verify_dalek(b: &mut Bencher) {
	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = Ed25519.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	let mut signature = vec![0u8; 64];

	key_pair.sign(&message, &mut signature);

	let mut public_key = vec![0u8; 32];

	key_pair.public_key(&mut public_key);

	let verifier = Ed25519
		.verifier_from_public_key(&public_key)
		.unwrap();

	verifier.verify(&message, &signature).unwrap();

	let run = || verifier.verify(&message, &signature);

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_verify_custom(b: &mut Bencher) {

	use std::str::FromStr;

	let path = utils::get_dylib("crypto_dylib_samples_dsa");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let dsa = DsaImpl::from_str(&path).unwrap();

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	let signature = key_pair.sign(&message);

	let verifier = dsa
		.verifier_from_public_key(&key_pair.public_key())
		.unwrap();

	verifier.verify(&message, &signature).unwrap();

	let run = || verifier.verify(&message, &signature);

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_verify_include_init_ring(b: &mut Bencher) {
	let dsa = DsaImpl::Ed25519;

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	let signature = key_pair.sign(&message);

	let run = || {
		let verifier = dsa
			.verifier_from_public_key(&key_pair.public_key())
			.unwrap();
		verifier.verify(&message, &signature)
	};

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_verify_include_init_dalek(b: &mut Bencher) {
	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = Ed25519.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	let mut signature = vec![0u8; 64];

	key_pair.sign(&message, &mut signature);

	let run = || {
		let mut public_key = vec![0; 32];
		key_pair.public_key(&mut public_key);
		let verifier = Ed25519
			.verifier_from_public_key(&public_key)
			.unwrap();
		verifier.verify(&message, &signature)
	};

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_ed25519_verify_include_init_custom(b: &mut Bencher) {

	use std::str::FromStr;

	let path = utils::get_dylib("crypto_dylib_samples_dsa");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let dsa = DsaImpl::from_str(&path).unwrap();

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	let message = [1u8; 256];

	let signature = key_pair.sign(&message);

	let verifier = dsa
		.verifier_from_public_key(&key_pair.public_key())
		.unwrap();

	verifier.verify(&message, &signature).unwrap();

	let run = || {
		let verifier = dsa
			.verifier_from_public_key(&key_pair.public_key())
			.unwrap();
		verifier.verify(&message, &signature).unwrap();
	};

	b.iter(|| black_box(run()));
}

#[bench]
fn bench_vec_copy(b: &mut Bencher) {
	let source = vec![1u8; 64];

	let run = || {
		let _clone = source.clone();
	};
	b.iter(|| black_box(run()));
}
