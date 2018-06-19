// Copyright 2017 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Light client data fetcher. Fetches requested data from remote full nodes.

use std::sync::Arc;
use futures::IntoFuture;
use heapsize::HeapSizeOf;

use primitives::block::{Header, HeaderHash, Id as BlockId, Number as BlockNumber};
use runtime_support::Hashable;
use state_machine::{CodeExecutor, read_proof_check};

use blockchain::HeaderBackend as BlockchainHeaderBackend;
use call_executor::CallResult;
use error::{Error as ClientError, ErrorKind as ClientErrorKind, Result as ClientResult};
use light::blockchain::{Blockchain, Storage as BlockchainStorage};
use light::call_executor::check_execution_proof;

/// Remote canonical header request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RemoteHeaderRequest {
	/// Number of the header to query.
	pub block: BlockNumber,
	/// Request retry count before failing. If None, default value is used.
	pub retry_count: Option<usize>,
}

/// Remote storage read request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RemoteReadRequest {
	/// Read at state of given block.
	pub block: HeaderHash,
	/// Storage key to read.
	pub key: Vec<u8>,
	/// Request retry count before failing. If None, default value is used.
	pub retry_count: Option<usize>,
}

/// Remote call request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RemoteCallRequest {
	/// Call at state of given block.
	pub block: HeaderHash,
	/// Method to call.
	pub method: String,
	/// Call data.
	pub call_data: Vec<u8>,
	/// Request retry count before failing. If None, default value is used.
	pub retry_count: Option<usize>,
}

/// Light client data fetcher. Implementations of this trait must check if remote data
/// is correct (see FetchedDataChecker) and return already checked data.
pub trait Fetcher: Send + Sync {
	/// Remote header future.
	type RemoteHeaderResult: IntoFuture<Item=Header, Error=ClientError>;
	/// Remote storage read future.
	type RemoteReadResult: IntoFuture<Item=Option<Vec<u8>>, Error=ClientError>;
	/// Remote call result future.
	type RemoteCallResult: IntoFuture<Item=CallResult, Error=ClientError>;

	/// Fetch remote header.
	fn remote_header(&self, request: RemoteHeaderRequest) -> Self::RemoteHeaderResult;
	/// Fetch remote storage value.
	fn remote_read(&self, request: RemoteReadRequest) -> Self::RemoteReadResult;
	/// Fetch remote call result.
	fn remote_call(&self, request: RemoteCallRequest) -> Self::RemoteCallResult;
}

/// Light client remote data checker.
pub trait FetchChecker: Send + Sync {
	/// Check remote header proof.
	fn check_header_proof(&self, request: &RemoteHeaderRequest, header: Header, remote_proof: Vec<Vec<u8>>) -> ClientResult<Header>;
	/// Check remote storage read proof.
	fn check_read_proof(&self, request: &RemoteReadRequest, remote_proof: Vec<Vec<u8>>) -> ClientResult<Option<Vec<u8>>>;
	/// Check remote method execution proof.
	fn check_execution_proof(&self, request: &RemoteCallRequest, remote_proof: Vec<Vec<u8>>) -> ClientResult<CallResult>;
}

/// Remote data checker.
pub struct LightDataChecker<S, E, F> {
	blockchain: Arc<Blockchain<S, F>>,
	executor: E,
}

impl<S, E, F> LightDataChecker<S, E, F> {
	/// Create new light data checker.
	pub fn new(blockchain: Arc<Blockchain<S, F>>, executor: E) -> Self {
		Self {
			blockchain,
			executor,
		}
	}

	/// Get blockchain reference.
	pub fn blockchain(&self) -> &Arc<Blockchain<S, F>> {
		&self.blockchain
	}
}

impl<S, E, F> FetchChecker for LightDataChecker<S, E, F>
	where
		S: BlockchainStorage,
		E: CodeExecutor,
		F: Fetcher,
{
	fn check_header_proof(&self, request: &RemoteHeaderRequest, header: Header, remote_proof: Vec<Vec<u8>>) -> ClientResult<Header> {
		let (cht_root, cht_key) = self.blockchain.storage().cht(request.block)?;
		let local_cht_value = read_proof_check(cht_root.into(), remote_proof, &cht_key).map_err(|e| ClientError::from(e))?;
		let local_cht_value = local_cht_value.ok_or_else(|| ClientErrorKind::InvalidHeaderProof)?;
		let local_hash = self.blockchain.storage().cht_decode_header_hash(&local_cht_value)?;
		let remote_hash = header.blake2_256().into();
		match local_hash == remote_hash {
			true => Ok(header),
			false => Err(ClientErrorKind::InvalidHeaderProof.into()),
		}
	}

	fn check_read_proof(&self, request: &RemoteReadRequest, remote_proof: Vec<Vec<u8>>) -> ClientResult<Option<Vec<u8>>> {
		let local_header = self.blockchain.header(BlockId::Hash(request.block))?;
		let local_header = local_header.ok_or_else(|| ClientErrorKind::UnknownBlock(BlockId::Hash(request.block)))?;
		let local_state_root = local_header.state_root;
		read_proof_check(local_state_root.0, remote_proof, &request.key).map_err(Into::into)
	}

	fn check_execution_proof(&self, request: &RemoteCallRequest, remote_proof: Vec<Vec<u8>>) -> ClientResult<CallResult> {
		check_execution_proof(&*self.blockchain, &self.executor, request, remote_proof)
	}
}

impl HeapSizeOf for RemoteHeaderRequest {
	fn heap_size_of_children(&self) -> usize {
		0
	}
}

impl HeapSizeOf for RemoteReadRequest {
	fn heap_size_of_children(&self) -> usize {
		self.block.heap_size_of_children() + self.key.heap_size_of_children()
	}
}

impl HeapSizeOf for RemoteCallRequest {
	fn heap_size_of_children(&self) -> usize {
		self.block.heap_size_of_children() + self.method.heap_size_of_children()
			+ self.call_data.heap_size_of_children()
	}
}