use crate::prelude::*;
use async_trait::async_trait;
use core::{convert::TryInto, fmt};
use jsonrpc::{
	error::{result_to_response, standard_error, RpcError, StandardError},
	serde_json,
	serde_json::{to_string, value::to_raw_value},
};
use serde_json::value::RawValue;
pub use surf::Url;

use crate::rpc::{self, Rpc, RpcResult};

#[derive(Debug)]
pub struct Backend(Url);

impl Backend {
	pub fn new<U>(url: U) -> Self
	where
		U: TryInto<Url>,
		<U as TryInto<Url>>::Error: fmt::Debug,
	{
		Backend(url.try_into().expect("Url"))
	}
}

#[async_trait]
impl Rpc for Backend {
	/// HTTP based JSONRpc request expecting an hex encoded result
	async fn rpc(&self, method: &str, params: Vec<Box<RawValue>>) -> RpcResult {
		log::info!("RPC `{}` to {}", method, &self.0);
		let req = surf::post(&self.0).content_type("application/json").body(
			to_string(&rpc::Request { id: 1.into(), jsonrpc: Some("2.0"), method, params })
				.unwrap(),
		);
		let client = surf::client().with(surf::middleware::Redirect::new(2));
		let mut res = client
			.send(req)
			.await
			.map_err(|err| rpc::Error::Transport(err.into_inner().into()))?;

		let status = res.status();
		let res = if status.is_success() {
			res.body_json::<rpc::Response>()
				.await
				.map_err(|err| {
					standard_error(
						StandardError::ParseError,
						Some(to_raw_value(&err.to_string()).unwrap()),
					)
				})?
				.result::<String>()?
		} else {
			log::debug!("RPC HTTP status: {}", res.status());
			let err = res.body_string().await.unwrap_or_else(|_| status.canonical_reason().into());
			let err = to_raw_value(&err).expect("error string");

			return Err(if status.is_client_error() {
				standard_error(StandardError::InvalidRequest, Some(err)).into()
			} else {
				standard_error(StandardError::InternalError, Some(err)).into()
			})
		};

		log::debug!("RPC Response: {}...", &res[..res.len().min(20)]);
		// assume the response is a hex encoded string starting with "0x"
		let response = hex::decode(&res[2..])
			.map_err(|_err| standard_error(StandardError::InternalError, None))?;
		Ok(response)
	}

	async fn rpc_single(
		&self,
		method: &str,
		params: Box<RawValue>,
	) -> Result<serde_json::value::Value, RpcError> {
		panic!()
	}
}
