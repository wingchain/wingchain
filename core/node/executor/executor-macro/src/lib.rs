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
use std::collections::{HashMap, HashSet};

use syn::{
	parse_macro_input, FnArg, Ident, ImplItem, ImplItemMethod, ItemImpl, Meta, NestedMeta, Type,
};

use quote::quote;

/// Module method validate_xxx will be treated as the validator of method xxx
const VALIDATE_METHOD_PREFIX: &'static str = "validate_";

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
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C, U>::META_MODULE) }
		})
		.collect::<Vec<_>>();

	let validate_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => {
				let result = module::#ident::Module::<C, U>::validate_call(util, &call);
				match result {
					Ok(result) => Ok(Ok(result)),
					Err(e) => match e {
						ModuleError::System(e) => Err(e),
						ModuleError::Application(e) => Ok(Err(e.to_string())),
					}
				}
			} }
		})
		.collect::<Vec<_>>();

	let is_write_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => Ok(module::#ident::Module::<C, U>::is_write_call(&call)) }
		})
		.collect::<Vec<_>>();

	let execute_call_ts_vec = variants
		.iter()
		.map(|x| {
			let ident = &x.ident;
			quote! { stringify!(#ident) => {
				let module = module::#ident::Module::<_, _>::new(context.clone(), util.clone());
				let result = module.execute_call(sender, call);
				match result {
					Ok(result) => Ok(Ok(result)),
					Err(e) => match e {
						ModuleError::System(e) => Err(e),
						ModuleError::Application(e) => Ok(Err(e.to_string())),
					}
				}
			} }
		})
		.collect::<Vec<_>>();

	let type_name = &ast.ident;
	let gen = quote! {
		struct #type_name;
		impl #type_name {
			fn is_meta<C: ContextT, U: UtilT>(module: &str) -> CommonResult<bool>{
				match module {
					#(#is_meta_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidTxModule(other.to_string()).into()),
				}
			}
			fn validate_call<C: ContextT, U: UtilT>(module: &str, util: &U, call: &Call) -> CommonResult<CallResult<()>>{
				match module {
					#(#validate_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidTxModule(other.to_string()).into()),
				}
			}
			fn is_write_call<C: ContextT, U: UtilT>(module: &str, call: &Call) -> CommonResult<Option<bool>> {
				match module {
					#(#is_write_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidTxModule(other.to_string()).into()),
				}
			}
			fn execute_call<C: ContextT, U: UtilT>(module: &str, context: &C, util: &U, sender: Option<&Address>, call: &Call) -> CommonResult<OpaqueCallResult>{
				match module {
					#(#execute_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidTxModule(other.to_string()).into()),
				}
			}
		}
	};
	gen.into()
}

#[proc_macro_attribute]
pub fn call(_attr: TokenStream, item: TokenStream) -> TokenStream {
	item
}

#[proc_macro_attribute]
pub fn module(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let impl_item = parse_macro_input!(item as ItemImpl);

	let type_name = get_module_ident(&impl_item);

	let methods = get_module_call_methods(&impl_item);

	let validate_call_ts_vec = methods
		.iter()
		.map(|x| {
			let method_ident = &x.method_ident;
			let params_ident = x.params_ident.clone();
			let validate = match &x.validate_ident {
				Some(validate_ident) => quote! {
					Self::#validate_ident(&util, params)
				},
				None => quote! {Ok(())},
			};
			quote! {
				stringify!(#method_ident) => {
					let params = match codec::decode::<#params_ident>(&params) {
						Ok(params) => params,
						Err(_) => return Err(errors::ErrorKind::InvalidTxParams("codec error".to_string()).into()),
					};
					#validate
				}
			}
		})
		.collect::<Vec<_>>();

	let is_write_call_ts_vec = methods
		.iter()
		.map(|x| {
			let method_ident = &x.method_ident;
			let is_write = x.write;
			quote! { stringify!(#method_ident) => Some(#is_write) }
		})
		.collect::<Vec<_>>();

	let execute_call_ts_vec = methods
		.iter()
		.map(|x| {
			let method_ident = &x.method_ident;
			quote! {
				stringify!(#method_ident) => {

					let params = match codec::decode(&params) {
						Ok(params) => params,
						Err(_) => return Err(errors::ErrorKind::InvalidTxParams("codec error".to_string()).into()),
					};
					let result = self.#method_ident(sender, params);

					// ModuleResult to OpaqueModuleResult
					let result = result?;
					let result = codec::encode(&result)?;
					Ok(result)
				}
			}
		})
		.collect::<Vec<_>>();

	let gen = quote! {

		#impl_item

		impl<C, U> ModuleT for #type_name<C, U> where C: Context, U: Util {

			const META_MODULE: bool = Self::META_MODULE;
			const STORAGE_KEY: &'static [u8] = Self::STORAGE_KEY;

			type C = C;
			type U = U;

			fn new(context: Self::C, util: Self::U) -> Self {
				Self::new(context, util)
			}

			fn validate_call(util: &Self::U, call: &Call) -> ModuleResult<()> {
				let params = &call.params.0[..];
				match call.method.as_str() {
					#(#validate_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidTxMethod(other.to_string()).into()),
				}
			}

			fn is_write_call(call: &Call) -> Option<bool> {
				let method = call.method.as_str();
				match method {
					#(#is_write_call_ts_vec),*,
					other => None,
				}
			}

			fn execute_call(&self, sender: Option<&Address>, call: &Call) -> OpaqueModuleResult {
				let params = &call.params.0[..];
				let method = call.method.as_str();
				match method {
					#(#execute_call_ts_vec),*,
					other => Err(errors::ErrorKind::InvalidTxMethod(other.to_string()).into()),
				}
			}

		}

	};
	gen.into()
}

fn get_module_ident(impl_item: &ItemImpl) -> Ident {
	match &*impl_item.self_ty {
		Type::Path(type_path) => type_path.path.segments[0].ident.clone(),
		_ => panic!("module ident not found"),
	}
}

fn get_module_call_methods(impl_item: &ItemImpl) -> Vec<ModuleMethod> {
	let call_methods = impl_item
		.items
		.iter()
		.filter_map(|item| {
			if let ImplItem::Method(method) = item {
				let call_method = method.attrs.iter().find_map(|attr| {
					let meta = attr.parse_meta().unwrap();
					match meta {
						Meta::Path(word) => {
							if word.segments[0].ident == "call" {
								Some(ModuleMethod {
									write: false,
									validate_ident: None,
									method_ident: method.sig.ident.clone(),
									params_ident: get_method_params_ident(&method),
									method: method.clone(),
								})
							} else {
								None
							}
						}
						Meta::List(list) => {
							if list.path.segments[0].ident == "call" {
								let write = list.nested.iter().find_map(|nm| match nm {
									NestedMeta::Meta(Meta::NameValue(nv))
										if nv.path.segments[0].ident == "write" =>
									{
										match &nv.lit {
											syn::Lit::Bool(value) => Some(value.value),
											_ => panic!("write should have a bool value"),
										}
									}
									_ => None,
								});
								let write = write.unwrap_or(false);
								Some(ModuleMethod {
									write,
									validate_ident: None,
									method_ident: method.sig.ident.clone(),
									params_ident: get_method_params_ident(&method),
									method: method.clone(),
								})
							} else {
								None
							}
						}
						_ => None,
					}
				});
				call_method
			} else {
				None
			}
		})
		.collect::<Vec<_>>();

	let call_method_names = call_methods
		.iter()
		.map(|x| x.method_ident.to_string())
		.collect::<HashSet<_>>();
	let call_method_validates = impl_item
		.items
		.iter()
		.filter_map(|item| {
			if let ImplItem::Method(method) = item {
				let method_name = method.sig.ident.to_string();
				if method_name.starts_with(VALIDATE_METHOD_PREFIX) {
					let call_method_name = method_name.trim_start_matches(VALIDATE_METHOD_PREFIX);
					if call_method_names.contains(call_method_name) {
						Some((call_method_name.to_string(), method.sig.ident.clone()))
					} else {
						None
					}
				} else {
					None
				}
			} else {
				None
			}
		})
		.collect::<HashMap<_, _>>();

	let call_methods = call_methods
		.into_iter()
		.map(|mut method| {
			let method_name = method.method_ident.to_string();
			method.validate_ident = call_method_validates.get(&method_name).cloned();
			method
		})
		.collect::<Vec<_>>();

	call_methods
}

fn get_method_params_ident(method: &ImplItemMethod) -> Ident {
	if method.sig.inputs.len() == 3 {
		let params = &method.sig.inputs[2];
		let pat_type = match params {
			FnArg::Typed(pat_type) => pat_type,
			_ => unreachable!(),
		};
		let params_ident = if let Type::Path(path) = &*pat_type.ty {
			path.path
				.get_ident()
				.expect("no params ident found")
				.clone()
		} else {
			panic!("no params type found")
		};
		params_ident
	} else {
		panic!("call method input should be (&self, sender: Option<&Address>, params: Type)");
	}
}

struct ModuleMethod {
	#[allow(dead_code)]
	method: ImplItemMethod,
	method_ident: Ident,
	params_ident: Ident,
	write: bool,
	validate_ident: Option<Ident>,
}
