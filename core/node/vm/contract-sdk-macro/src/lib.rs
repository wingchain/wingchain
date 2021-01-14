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

use syn::{
	parse_macro_input, FnArg, Ident, ImplItem, ImplItemMethod, ItemImpl, Meta, NestedMeta, Type,
};

use quote::quote;
use std::collections::{HashMap, HashSet};

/// Contract method validate_xxx will be treated as the validator of method xxx
const VALIDATE_METHOD_PREFIX: &str = "validate_";

#[proc_macro_attribute]
pub fn call(_attr: TokenStream, item: TokenStream) -> TokenStream {
	item
}

#[proc_macro_attribute]
pub fn init(_attr: TokenStream, item: TokenStream) -> TokenStream {
	item
}

#[proc_macro_attribute]
pub fn contract(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let impl_item = parse_macro_input!(item as ItemImpl);

	let type_name = get_module_ident(&impl_item);

	let init_methods = get_module_init_methods(&impl_item);
	let call_methods = get_module_call_methods(&impl_item);

	let get_validate_ts_vec = |methods: &Vec<ModuleMethod>| {
		methods
			.iter()
			.map(|x| {
				let method_ident = &x.method_ident;
				let params_ident = x.params_ident.clone();
				let is_payable = x.payable;
				let validate = match &x.validate_ident {
					Some(validate_ident) => quote! {
					self.#validate_ident(params)
				},
					None => quote! {Ok(())},
				};
				quote! {
					stringify!(#method_ident) => {
						if pay_value > 0 {
							if !#is_payable {
								return Err(ContractError::NotPayable);
							}
						}

						let params : #params_ident = serde_json::from_slice(&params).map_err(|_|ContractError::InvalidParams)?;
						#validate
					},
				}
			})
			.collect::<Vec<_>>()
	};

	let validate_init_ts_vec = get_validate_ts_vec(&init_methods);
	let validate_call_ts_vec = get_validate_ts_vec(&call_methods);

	let get_execute_ts_vec = |methods: &Vec<ModuleMethod>| {
		methods
			.iter()
			.map(|x| {
				let method_ident = &x.method_ident;
				let is_payable = x.payable;
				quote! {
					stringify!(#method_ident) => {
						if pay_value > 0 {
							if #is_payable {
								import::pay();
							} else{
								return Err(ContractError::NotPayable);
							}
						}

						let params = serde_json::from_slice(&params).map_err(|_|ContractError::InvalidParams)?;
						let result = self.#method_ident(params)?;
						let result = serde_json::to_vec(&result).map_err(|_|ContractError::Serialize)?;
						Ok(result)
					},
				}
			})
			.collect::<Vec<_>>()
	};

	let execute_init_ts_vec = get_execute_ts_vec(&init_methods);
	let execute_call_ts_vec = get_execute_ts_vec(&call_methods);

	let gen = quote! {

		#impl_item

		impl #type_name {

			fn validate_init(&self, method: &str, params: Vec<u8>, pay_value: Balance) -> ContractResult<()> {
				match method {
					#(#validate_init_ts_vec)*
					other => Err(ContractError::InvalidMethod),
				}
			}

			fn validate_call(&self, method: &str, params: Vec<u8>, pay_value: Balance) -> ContractResult<()> {
				match method {
					#(#validate_call_ts_vec)*
					other => Err(ContractError::InvalidMethod),
				}
			}

			fn execute_init(&self, method: &str, params: Vec<u8>, pay_value: Balance) -> ContractResult<Vec<u8>> {
				match method {
					#(#execute_init_ts_vec)*
					other => Err(ContractError::InvalidMethod),
				}
			}

			fn execute_call(&self, method: &str, params: Vec<u8>, pay_value: Balance) -> ContractResult<Vec<u8>> {
				match method {
					#(#execute_call_ts_vec)*
					other => Err(ContractError::InvalidMethod),
				}
			}
		}

		fn get_method() -> ContractResult<String> {
			let share_id = std::ptr::null::<u8>() as u64;
			import::method_read(share_id);
			let len = import::share_len(share_id);
			let method = vec![0u8; len as usize];
			import::share_read(share_id, method.as_ptr() as _);
			let method = String::from_utf8(method).map_err(|_|ContractError::InvalidMethod)?;
			Ok(method)
		}

		fn get_params() -> ContractResult<Vec<u8>> {
			let share_id = std::ptr::null::<u8>() as u64;
			import::params_read(share_id);
			let len = import::share_len(share_id);
			let params = vec![0u8; len as usize];
			import::share_read(share_id, params.as_ptr() as _);
			let params = if params.len() == 0 { "null".as_bytes().to_vec() } else { params };

			Ok(params)
		}

		fn get_pay_value() -> ContractResult<Balance> {
			let pay_value = import::pay_value_read();
			Ok(pay_value)
		}

		fn set_result(result: ContractResult<()>) {
			match result {
				Ok(_) => (),
				Err(e) => {
					let error = e.to_string().into_bytes();
					import::error_return(error.len() as _, error.as_ptr() as _);
				}
			}
		}

		fn inner_validate_init() -> ContractResult<()> {
			let method = get_method()?;
			let params = get_params()?;
			let pay_value = get_pay_value()?;

			let contract = #type_name::new()?;
			contract.validate_init(&method, params, pay_value)?;

			Ok(())
		}

		fn inner_validate_call() -> ContractResult<()> {
			let method = get_method()?;
			let params = get_params()?;
			let pay_value = get_pay_value()?;

			let contract = #type_name::new()?;
			contract.validate_call(&method, params, pay_value)?;

			Ok(())
		}

		fn inner_execute_init() -> ContractResult<()> {
			let method = get_method()?;
			let params = get_params()?;
			let pay_value = get_pay_value()?;

			let contract = #type_name::new()?;
			let result = contract.execute_init(&method, params, pay_value)?;
			import::result_write(result.len() as _, result.as_ptr() as _);

			Ok(())
		}

		fn inner_execute_call() -> ContractResult<()> {
			let method = get_method()?;
			let params = get_params()?;
			let pay_value = get_pay_value()?;

			let contract = #type_name::new()?;
			let result = contract.execute_call(&method, params, pay_value)?;
			import::result_write(result.len() as _, result.as_ptr() as _);

			Ok(())
		}

		#[wasm_bindgen]
		pub fn validate_init() {
			let result = inner_validate_init();
			set_result(result);
		}

		#[wasm_bindgen]
		pub fn validate_call() {
			let result = inner_validate_call();
			set_result(result);
		}

		#[wasm_bindgen]
		pub fn execute_init() {
			let result = inner_execute_init();
			set_result(result);
		}

		#[wasm_bindgen]
		pub fn execute_call() {
			let result = inner_execute_call();
			set_result(result);
		}
	};
	gen.into()
}

fn get_module_ident(impl_item: &ItemImpl) -> Ident {
	match &*impl_item.self_ty {
		Type::Path(type_path) => type_path.path.segments[0].ident.clone(),
		_ => panic!("Module ident not found"),
	}
}

fn get_module_init_methods(impl_item: &ItemImpl) -> Vec<ModuleMethod> {
	get_module_methods_by_name(impl_item, "init")
}

fn get_module_call_methods(impl_item: &ItemImpl) -> Vec<ModuleMethod> {
	get_module_methods_by_name(impl_item, "call")
}

fn get_module_methods_by_name(impl_item: &ItemImpl, name: &str) -> Vec<ModuleMethod> {
	let methods = impl_item
		.items
		.iter()
		.filter_map(|item| {
			if let ImplItem::Method(method) = item {
				let call_method = method.attrs.iter().find_map(|attr| {
					let meta = attr.parse_meta().unwrap();
					match meta {
						Meta::Path(word) => {
							if word.segments[0].ident == name {
								Some(ModuleMethod {
									payable: false,
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
							if list.path.segments[0].ident == name {
								let payable = list.nested.iter().find_map(|nm| match nm {
									NestedMeta::Meta(Meta::NameValue(nv))
										if nv.path.segments[0].ident == "payable" =>
									{
										match &nv.lit {
											syn::Lit::Bool(value) => Some(value.value),
											_ => panic!("Payable should have a bool value"),
										}
									}
									_ => None,
								});
								let payable = payable.unwrap_or(false);
								Some(ModuleMethod {
									payable,
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

	let method_names = methods
		.iter()
		.map(|x| x.method_ident.to_string())
		.collect::<HashSet<_>>();
	let method_validates = impl_item
		.items
		.iter()
		.filter_map(|item| {
			if let ImplItem::Method(method) = item {
				let method_name = method.sig.ident.to_string();
				if method_name.starts_with(VALIDATE_METHOD_PREFIX) {
					let method_name = method_name.trim_start_matches(VALIDATE_METHOD_PREFIX);
					if method_names.contains(method_name) {
						Some((method_name.to_string(), method.sig.ident.clone()))
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

	methods
		.into_iter()
		.map(|mut method| {
			let method_name = method.method_ident.to_string();
			method.validate_ident = method_validates.get(&method_name).cloned();
			method
		})
		.collect::<Vec<_>>()
}

fn get_method_params_ident(method: &ImplItemMethod) -> Ident {
	if method.sig.inputs.len() == 2 {
		let params = &method.sig.inputs[1];
		let pat_type = match params {
			FnArg::Typed(pat_type) => pat_type,
			_ => unreachable!(),
		};
		let params_ident = if let Type::Path(path) = &*pat_type.ty {
			path.path
				.get_ident()
				.expect("No params ident found")
				.clone()
		} else {
			panic!("No params type found")
		};
		params_ident
	} else {
		panic!("Call method args should be (&self, params: Type)");
	}
}

struct ModuleMethod {
	#[allow(dead_code)]
	method: ImplItemMethod,
	method_ident: Ident,
	params_ident: Ident,
	payable: bool,
	validate_ident: Option<Ident>,
}
