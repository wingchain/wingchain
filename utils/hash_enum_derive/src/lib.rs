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

use crypto::blake2b;
use heck::SnakeCase;
use proc_macro2::Ident;
use quote::quote;
use syn::export::TokenStream;

#[proc_macro_derive(HashEnum)]
pub fn hash_enum(ts: TokenStream) -> TokenStream {
	let ast: syn::DeriveInput = syn::parse(ts).unwrap();

	let variants = match ast.data {
		syn::Data::Enum(ref v) => &v.variants,
		_ => panic!("HashEnum only works on Enums"),
	};
	let type_name = &ast.ident;

	let from_hash_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			let hash = get_hash(ident);
			quote! { &[#(#hash),*] => Some(#type_name::#ident) }
		})
		.collect::<Vec<_>>();

	let from_name_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			let name = get_name(ident);
			quote! { #name => Some(#type_name::#ident) }
		})
		.collect::<Vec<_>>();

	let hash_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			let hash = get_hash(ident);
			quote! { #type_name::#ident => &[#(#hash),*] }
		})
		.collect::<Vec<_>>();

	let name_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			let name = get_name(ident);
			quote! { #type_name::#ident => #name }
		})
		.collect::<Vec<_>>();

	let gen = quote! {
		impl HashEnum for #type_name {
			fn from_hash(hash: &[u8]) -> Option<Self> {
				match hash {
					#(#from_hash_ts_vec),*,
					_ => None,
				}
			}
			fn from_name(name: &str) -> Option<Self> {
				match name {
					#(#from_name_ts_vec),*,
					_ => None,
				}
			}
			fn hash(&self) -> &[u8] {
				match *self {
					#(#hash_ts_vec),*
				}
			}
			fn name(&self) -> &str {
				match *self {
					#(#name_ts_vec),*
				}
			}
		}
	};
	gen.into()
}

fn get_name(ident: &Ident) -> String {
	ident.to_string().to_snake_case()
}

fn get_hash(ident: &Ident) -> Vec<u8> {
	let mut out = [0u8; 32];
	blake2b::Blake2b::blake2b(&mut out, get_name(ident).as_bytes(), &[]);
	let hash = out[0..4].to_vec();
	hash
}
