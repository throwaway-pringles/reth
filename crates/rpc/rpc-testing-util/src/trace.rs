//! Helpers for testing trace calls.
use futures::{Stream, StreamExt};
use jsonrpsee::core::Error as RpcError;
use reth_primitives::{BlockId, Bytes, TxHash};
use reth_rpc_api::clients::TraceApiClient;
use reth_rpc_types::{
    trace::parity::{LocalizedTransactionTrace, TraceResults, TraceType},
    CallRequest,
};
use std::{
    collections::HashSet,
    pin::Pin,
    task::{Context, Poll},
};
/// A type alias that represents the result of a raw transaction trace stream.
type RawTransactionTraceResult<'a> =
    Pin<Box<dyn Stream<Item = Result<(TraceResults, Bytes), (RpcError, Bytes)>> + 'a>>;
/// A result type for the `trace_block` method that also captures the requested block.
pub type TraceBlockResult = Result<(Vec<LocalizedTransactionTrace>, BlockId), (RpcError, BlockId)>;
/// Type alias representing the result of replaying a transaction.

pub type ReplayTransactionResult = Result<(TraceResults, TxHash), (RpcError, TxHash)>;

/// A type representing the result of calling `trace_call_many` method.

pub type CallManyTraceResult = Result<
    (Vec<TraceResults>, Vec<(CallRequest, HashSet<TraceType>)>),
    (RpcError, Vec<(CallRequest, HashSet<TraceType>)>),
>;

/// An extension trait for the Trace API.
#[async_trait::async_trait]
pub trait TraceApiExt {
    /// The provider type that is used to make the requests.
    type Provider;

    /// Returns a new stream that yields the traces for the given blocks.
    ///
    /// See also [StreamExt::buffered].
    fn trace_block_buffered<I, B>(&self, params: I, n: usize) -> TraceBlockStream<'_>
    where
        I: IntoIterator<Item = B>,
        B: Into<BlockId>;

    /// Returns a new stream that yields the traces for the given blocks.
    ///
    /// See also [StreamExt::buffer_unordered].
    fn trace_block_buffered_unordered<I, B>(&self, params: I, n: usize) -> TraceBlockStream<'_>
    where
        I: IntoIterator<Item = B>,
        B: Into<BlockId>;

    /// Returns a new stream that replays the transactions for the given transaction hashes.
    ///
    /// This returns all results in order.
    fn replay_transactions<I>(
        &self,
        tx_hashes: I,
        trace_types: HashSet<TraceType>,
    ) -> ReplayTransactionStream<'_>
    where
        I: IntoIterator<Item = TxHash>;

    /// Returns a new stream that traces the provided raw transaction data.
    fn trace_raw_transaction_stream(
        &self,
        data: Bytes,
        trace_types: HashSet<TraceType>,
        block_id: Option<BlockId>,
    ) -> RawTransactionTraceStream<'_>;
    /// Creates a stream of results for multiple dependent transaction calls on top of the same
    /// block.

    fn trace_call_many_stream<I>(
        &self,
        calls: I,
        block_id: Option<BlockId>,
    ) -> CallManyTraceStream<'_>
    where
        I: IntoIterator<Item = (CallRequest, HashSet<TraceType>)>;
}
/// A stream that provides asynchronous iteration over results from the `trace_call_many` function.
///
/// The stream yields items of type `CallManyTraceResult`.
#[must_use = "streams do nothing unless polled"]
pub struct CallManyTraceStream<'a> {
    stream: Pin<Box<dyn Stream<Item = CallManyTraceResult> + 'a>>,
}

impl<'a> Stream for CallManyTraceStream<'a> {
    type Item = CallManyTraceResult;
    /// Polls for the next item from the stream.

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl<'a> std::fmt::Debug for CallManyTraceStream<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallManyTraceStream").finish()
    }
}

/// A stream that traces the provided raw transaction data.

#[must_use = "streams do nothing unless polled"]
pub struct RawTransactionTraceStream<'a> {
    stream: RawTransactionTraceResult<'a>,
}

impl<'a> Stream for RawTransactionTraceStream<'a> {
    type Item = Result<(TraceResults, Bytes), (RpcError, Bytes)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl<'a> std::fmt::Debug for RawTransactionTraceStream<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawTransactionTraceStream").finish()
    }
}

/// A stream that replays the transactions for the requested hashes.
#[must_use = "streams do nothing unless polled"]
pub struct ReplayTransactionStream<'a> {
    stream: Pin<Box<dyn Stream<Item = ReplayTransactionResult> + 'a>>,
}
impl<'a> Stream for ReplayTransactionStream<'a> {
    type Item = ReplayTransactionResult;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl<'a> std::fmt::Debug for ReplayTransactionStream<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplayTransactionStream").finish()
    }
}

#[async_trait::async_trait]
impl<T: TraceApiClient + Sync> TraceApiExt for T {
    type Provider = T;

    fn trace_block_buffered<I, B>(&self, params: I, n: usize) -> TraceBlockStream<'_>
    where
        I: IntoIterator<Item = B>,
        B: Into<BlockId>,
    {
        let blocks = params.into_iter().map(|b| b.into()).collect::<Vec<_>>();
        let stream = futures::stream::iter(blocks.into_iter().map(move |block| async move {
            match self.trace_block(block).await {
                Ok(result) => Ok((result.unwrap_or_default(), block)),
                Err(err) => Err((err, block)),
            }
        }))
        .buffered(n);
        TraceBlockStream { stream: Box::pin(stream) }
    }

    fn trace_block_buffered_unordered<I, B>(&self, params: I, n: usize) -> TraceBlockStream<'_>
    where
        I: IntoIterator<Item = B>,
        B: Into<BlockId>,
    {
        let blocks = params.into_iter().map(|b| b.into()).collect::<Vec<_>>();
        let stream = futures::stream::iter(blocks.into_iter().map(move |block| async move {
            match self.trace_block(block).await {
                Ok(result) => Ok((result.unwrap_or_default(), block)),
                Err(err) => Err((err, block)),
            }
        }))
        .buffer_unordered(n);
        TraceBlockStream { stream: Box::pin(stream) }
    }

    fn replay_transactions<I>(
        &self,
        tx_hashes: I,
        trace_types: HashSet<TraceType>,
    ) -> ReplayTransactionStream<'_>
    where
        I: IntoIterator<Item = TxHash>,
    {
        let hashes = tx_hashes.into_iter().collect::<Vec<_>>();
        let stream = futures::stream::iter(hashes.into_iter().map(move |hash| {
            let trace_types_clone = trace_types.clone(); // Clone outside of the async block
            async move {
                match self.replay_transaction(hash, trace_types_clone).await {
                    Ok(result) => Ok((result, hash)),
                    Err(err) => Err((err, hash)),
                }
            }
        }))
        .buffered(10);
        ReplayTransactionStream { stream: Box::pin(stream) }
    }
    fn trace_raw_transaction_stream(
        &self,
        data: Bytes,
        trace_types: HashSet<TraceType>,
        block_id: Option<BlockId>,
    ) -> RawTransactionTraceStream<'_> {
        let stream = futures::stream::once(async move {
            match self.trace_raw_transaction(data.clone(), trace_types, block_id).await {
                Ok(result) => Ok((result, data)),
                Err(err) => Err((err, data)),
            }
        });
        RawTransactionTraceStream { stream: Box::pin(stream) }
    }

    fn trace_call_many_stream<I>(
        &self,
        calls: I,
        block_id: Option<BlockId>,
    ) -> CallManyTraceStream<'_>
    where
        I: IntoIterator<Item = (CallRequest, HashSet<TraceType>)>,
    {
        let call_set = calls.into_iter().collect::<Vec<_>>();
        let stream = futures::stream::once(async move {
            match self.trace_call_many(call_set.clone(), block_id).await {
                Ok(results) => Ok((results, call_set)),
                Err(err) => Err((err, call_set)),
            }
        });
        CallManyTraceStream { stream: Box::pin(stream) }
    }
}

/// A stream that yields the traces for the requested blocks.
#[must_use = "streams do nothing unless polled"]
pub struct TraceBlockStream<'a> {
    stream: Pin<Box<dyn Stream<Item = TraceBlockResult> + 'a>>,
}

impl<'a> TraceBlockStream<'a> {
    /// Returns the next error result of the stream.
    pub async fn next_err(&mut self) -> Option<(RpcError, BlockId)> {
        loop {
            match self.next().await? {
                Ok(_) => continue,
                Err(err) => return Some(err),
            }
        }
    }
}

impl<'a> Stream for TraceBlockStream<'a> {
    type Item = TraceBlockResult;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl<'a> std::fmt::Debug for TraceBlockStream<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TraceBlockStream").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::http_client::HttpClientBuilder;
    use reth_primitives::BlockNumberOrTag;
    use std::collections::HashSet;

    fn assert_is_stream<St: Stream>(_: &St) {}

    #[tokio::test]
    async fn can_create_block_stream() {
        let client = HttpClientBuilder::default().build("http://localhost:8545").unwrap();
        let block = vec![BlockId::Number(5u64.into()), BlockNumberOrTag::Latest.into()];
        let stream = client.trace_block_buffered(block, 2);
        assert_is_stream(&stream);
    }

    #[tokio::test]
    #[ignore]
    async fn can_create_replay_transaction_stream() {
        let client = HttpClientBuilder::default().build("http://localhost:8545").unwrap();

        // Assuming you have some transactions you want to test, replace with actual hashes.
        let transactions = vec![
            "0x4e08fe36db723a338e852f89f613e606b0c9a17e649b18b01251f86236a2cef3".parse().unwrap(),
            "0xea2817f1aeeb587b82f4ab87a6dbd3560fc35ed28de1be280cb40b2a24ab48bb".parse().unwrap(),
        ];

        let trace_types = HashSet::from([TraceType::StateDiff, TraceType::VmTrace]);

        let mut stream = client.replay_transactions(transactions, trace_types);
        let mut successes = 0;
        let mut failures = 0;
        let mut all_results = Vec::new();

        assert_is_stream(&stream);

        while let Some(result) = stream.next().await {
            match result {
                Ok((trace_result, tx_hash)) => {
                    println!("Success for tx_hash {:?}: {:?}", tx_hash, trace_result);
                    successes += 1;
                    all_results.push(Ok((trace_result, tx_hash)));
                }
                Err((error, tx_hash)) => {
                    println!("Error for tx_hash {:?}: {:?}", tx_hash, error);
                    failures += 1;
                    all_results.push(Err((error, tx_hash)));
                }
            }
        }

        println!("Total successes: {}", successes);
        println!("Total failures: {}", failures);
    }

    #[tokio::test]
    #[ignore]
    async fn can_create_trace_call_many_stream() {
        let client = HttpClientBuilder::default().build("http://localhost:8545").unwrap();

        let call_request_1 = CallRequest::default();
        let call_request_2 = CallRequest::default();
        let trace_types = HashSet::from([TraceType::StateDiff, TraceType::VmTrace]);
        let calls = vec![(call_request_1, trace_types.clone()), (call_request_2, trace_types)];

        let mut stream = client.trace_call_many_stream(calls, None);

        assert_is_stream(&stream);

        while let Some(result) = stream.next().await {
            match result {
                Ok(trace_result) => {
                    println!("Success: {:?}", trace_result);
                }
                Err(error) => {
                    println!("Error: {:?}", error);
                }
            }
        }
    }
}
