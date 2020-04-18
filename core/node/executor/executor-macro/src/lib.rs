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
pub fn dispatcher(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let ast: syn::DeriveInput = syn::parse(item).unwrap();

	let variants = match ast.data {
		syn::Data::Enum(ref v) => &v.variants,
		_ => panic!("dispatcher only works on Enum"),
	};

	let is_meta_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C>::META_MODULE) }
		})
		.collect::<Vec<_>>();

	let is_valid_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C>::is_valid_call(&call)) }
		})
		.collect::<Vec<_>>();

	let is_write_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C>::is_write_call(&call)) }
		})
		.collect::<Vec<_>>();

	let execute_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => {
				let module = module::#ident::Module::<_>::new(context.clone());
				let result = module.execute_call(call);
				Ok(result)
			} }
		})
		.collect::<Vec<_>>();

	let type_name = &ast.ident;
	let gen = quote! {
		struct #type_name;
		impl #type_name {
			fn is_meta<C: ContextT>(module: &str) -> CommonResult<bool>{
				match module {
					#(#is_meta_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
			fn is_valid_call<C: ContextT>(module: &str, call: &Call) -> CommonResult<bool>{
				match module {
					#(#is_valid_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
			fn is_write_call<C: ContextT>(module: &str, call: &Call) -> CommonResult<Option<bool>> {
				match module {
					#(#is_write_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
			fn execute_call<C: ContextT>(module: &str, context: &C, call: &Call) -> CommonResult<CommonResult<()>>{
				match module {
					#(#execute_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidModule(other.to_string()).into()),
				}
			}
		}
	};
	gen.into()
}