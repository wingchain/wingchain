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

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn enum_codec(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let ast: syn::DeriveInput = syn::parse(item).unwrap();

	let variants = match ast.data {
		syn::Data::Enum(ref v) => &v.variants,
		_ => panic!("enum_codec only works on Enum"),
	};
	let type_name = &ast.ident;
	let encode_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { #type_name :: #ident(v) => {
				stringify!(#ident).encode_to(&mut r);
				v.encode_to(&mut r);
			}, }
		})
		.collect::<Vec<_>>();

	let decode_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => {
				Ok(Self::#ident(Decode::decode(value)?))
			}, }
		})
		.collect::<Vec<_>>();

	let gen = quote! {
		#ast
		impl Encode for #type_name {
			fn encode(&self) -> Vec<u8> {
				let mut r = vec![];
				match self {
					#(#encode_vec)*
				}
				r
			}
		}
		impl Decode for #type_name {
			fn decode<I: scale_codec::Input>(value: &mut I) -> Result<Self, scale_codec::Error> {
				let name: String = Decode::decode(value)?;
				match name.as_str() {
					#(#decode_vec)*
					_ => Err("Unknown variant name".into()),
				}
			}
		}
	};
	gen.into()
}
