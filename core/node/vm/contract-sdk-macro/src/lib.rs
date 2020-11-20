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

#[proc_macro_attribute]
pub fn call(_attr: TokenStream, item: TokenStream) -> TokenStream {
	item
}

#[proc_macro_attribute]
pub fn contract(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let item_copy = item.clone();
	let impl_item = parse_macro_input!(item_copy as ItemImpl);

	let type_name = get_module_ident(&impl_item);

	let methods = get_module_call_methods(&impl_item);

	let execute_call_ts_vec = methods
		.iter()
		.map(|x| {
			let method_ident = &x.method_ident;
			let is_payable = x.payable;
			quote! {
				stringify!(#method_ident) => {

					let contract_env = self.context.contract_env()?;
					let pay_value = contract_env.pay_value;
					if pay_value > 0 {
						if #is_payable {
							import::pay();
						} else{
							return Err(ContractError::NotPayable);
						}
					}

					let params = serde_json::from_slice(&params).map_err(|_|ContractError::Deserialize)?;
					let result = self.#method_ident(params)?;
					let result = serde_json::to_vec(&result).map_err(|_|ContractError::Serialize)?;
					Ok(result)
				}
			}
		})
		.collect::<Vec<_>>();

	let gen = quote! {

		#impl_item

		impl #type_name {
			fn execute_call(&self, method: Vec<u8>, params: Vec<u8>) -> ContractResult<Vec<u8>> {
				let method = String::from_utf8(method).map_err(|_|ContractError::Deserialize)?;
				let method = method.as_str();
				match method {
					#(#execute_call_ts_vec),*,
					other => Err(ContractError::InvalidMethod),
				}
			}
		}

		fn inner_execute_call() -> ContractResult<()> {
			let len = import::method_len();
			let method = vec![0u8; len as usize];
			import::method_read(method.as_ptr() as _);

			let len = import::input_len();
			let input = vec![0u8; len as usize];
			import::input_read(input.as_ptr() as _);
			let input = if input.len() == 0 { "null".as_bytes().to_vec() } else { input };

			let contract = #type_name::new()?;
			let output = contract.execute_call(method, input)?;
			import::output_write(output.len() as _, output.as_ptr() as _);

			Ok(())
		}

		#[wasm_bindgen]
		pub fn execute_call() {
			let result = inner_execute_call();
			match result {
				Ok(result) => (),
				Err(e) => {
					let error = e.to_string().into_bytes();
					import::error_return(error.len() as _, error.as_ptr() as _);
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
									payable: false,
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
								let payable = list.nested.iter().find_map(|nm| match nm {
									NestedMeta::Meta(Meta::NameValue(nv))
										if nv.path.segments[0].ident == "payable" =>
									{
										match &nv.lit {
											syn::Lit::Bool(value) => Some(value.value),
											_ => panic!("write should have a bool value"),
										}
									}
									_ => None,
								});
								let payable = payable.unwrap_or(false);
								Some(ModuleMethod {
									payable,
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

	call_methods
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
				.expect("no params ident found")
				.clone()
		} else {
			panic!("no params type found")
		};
		params_ident
	} else {
		panic!("call method input should be (&self, params: Type)");
	}
}

struct ModuleMethod {
	#[allow(dead_code)]
	method: ImplItemMethod,
	method_ident: Ident,
	#[allow(dead_code)]
	params_ident: Ident,
	payable: bool,
}
