# CodeIndex

CodeIndex is a local-first high performance codebase index engine designed for AI. It helps your LLM understand the structure and semantics of a codebase and grab code context when needed. CodexIndex is built on top of [qdrant](https://github.com/qdrant/qdrant) ([See crates/qdrant](./crates/qdrant)), a high performance vector database written in Rust.

## Features

- **🔐 Privacy first:** All data is stored locally on your machine.
- **🚀 High performance:** Indexing a normal codebase takes in seconds.
- **🤖 AI ready:** The index is designed for AI, which means it's easy to provide codebase context to your LLM.
- **⏰ Real-time:** The index is designed to be real-time. It can update indexes while you are typing.
- **⚙️ Configurable:** The index is designed to be configurable. You can customize the engine to fit your needs.

## Preview

```bash
[2023-09-04T10:21:20.246Z INFO  storage::content_manager::consensus::persistent] Loading raft state from ./storage/raft_state.json
[2023-09-04T10:21:20.248Z DEBUG storage::content_manager::consensus::persistent] State: Persistent { state: RaftState { hard_state: HardState { term: 0, vote: 0, commit: 0 }, conf_state: ConfState { voters: [7252149026178447], learners: [], voters_outgoing: [], learners_next: [], auto_leave: false } }, latest_snapshot_meta: SnapshotMetadataSer { term: 0, index: 0 }, apply_progress_queue: EntryApplyProgressQueue(None), peer_address_by_id: RwLock { data: {} }, this_peer_id: 7252149026178447, path: "./storage/raft_state.json", dirty: false }
[2023-09-04T10:21:20.251Z INFO  qdrant] Distributed mode disabled
[2023-09-04T10:21:20.251Z INFO  qdrant] Telemetry reporting enabled, id: 865ffc9a-a8e2-48b7-97f9-d62131d1ae77
[2023-09-04T10:21:20.251Z DEBUG qdrant] Waiting for thread web to finish
[2023-09-04T10:21:20.251Z INFO  qdrant::tonic] Qdrant gRPC listening on 6334
[2023-09-04T10:21:20.251Z INFO  qdrant::tonic] TLS disabled for gRPC API
[2023-09-04T10:21:20.252Z INFO  qdrant::actix] TLS disabled for REST API
[2023-09-04T10:21:20.252Z INFO  qdrant::actix] Qdrant HTTP listening on 6333
[2023-09-04T10:21:20.252Z INFO  actix_server::builder] starting 5 workers
[2023-09-04T10:21:20.252Z INFO  actix_server::server] Actix runtime found; starting in Actix runtime
[2023-09-04T10:21:20.254Z DEBUG reqwest::connect] starting new connection: https://staging-telemetry.qdrant.io/
[2023-09-04T10:21:20.254Z DEBUG reqwest::connect] proxy(http://127.0.0.1:7890) intercepts 'https://staging-telemetry.qdrant.io/'
[2023-09-04T10:21:20.254Z DEBUG hyper::client::connect::http] connecting to 127.0.0.1:7890
[2023-09-04T10:21:20.254Z DEBUG hyper::client::connect::http] connected to 127.0.0.1:7890
[2023-09-04T10:21:20.255Z DEBUG rustls::client::hs] No cached session for DnsName("staging-telemetry.qdrant.io")
[2023-09-04T10:21:20.255Z DEBUG rustls::client::hs] Not resuming any session
[2023-09-04T10:21:20.773Z DEBUG rustls::client::hs] ALPN protocol is Some(b"h2")
[2023-09-04T10:21:20.773Z DEBUG rustls::client::hs] Using ciphersuite TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
[2023-09-04T10:21:20.773Z DEBUG rustls::client::tls12::server_hello] Server supports tickets
[2023-09-04T10:21:20.773Z DEBUG rustls::client::tls12] ECDHE curve is ECParameters { curve_type: NamedCurve, named_group: secp256r1 }
[2023-09-04T10:21:20.773Z DEBUG rustls::client::tls12] Server DNS name is DnsName("staging-telemetry.qdrant.io")
[2023-09-04T10:21:20.992Z DEBUG hyper::client::pool] pooling idle connection for ("https", staging-telemetry.qdrant.io)
[2023-09-04T10:21:21.259Z DEBUG rustls::common_state] Sending warning alert CloseNotify
[2023-09-04T10:21:21.259Z INFO codeindex] Indexing: / 538188 files, 1438298 Symbols, 52G ... \
```

## APIs

### Javascript/Typescript SDK

#### Index

```typescript
import { CodeIndex } from '@askcodebase/code-index';

const CODE_INDEX_DEFAULT_ENDPOINT = 'http://localhost:52050';
const INDEX_KEY = 'my-index';
const codeIndex = new CodeIndex(INDEX_KEY);

// Index a codebase by path
const indexes1 = await codeIndex.index(CODE_INDEX_DEFAULT_ENDPOINT, {
  path: '/path/to/codebase',
  recursive: true,
  ignore: ['node_modules', 'dist'],
});

// Index a codebase by manual
const indexes2 = await codeIndex.index({
  files: [
    {
      path: '/path/to/codebase/file1.js',
      content: '...',
    },
    {
      path: '/path/to/codebase/file2.js',
      content: '...',
    },
  ],
});
```

#### Query

```typescript
const outline = await codeIndex.getOutline('main.py')
const callgraph = await codeIndex.getCallGraph('main.py')
const references = await codeIndex.getReferences('main.py')
const definitions = await codeIndex.getDefinitions('main.py')
const implementations = await codeIndex.getImplementations('main.py')
const typeDefinitions = await codeIndex.getTypeDefinitions('main.py')
const diagnostics = await codeIndex.getDiagnostics('main.py')
const documentLinks = await codeIndex.getDocumentLinks('main.py')
const symbols = await codeIndex.querySymbol('main.py', {
  position: {
    line: 1,
    character: 1,
  },
})
```


## Concepts

1. [ctags](https://github.com/universal-ctags/ctags)
2. LSP (Language Server Protocol)
3. [tree-sitter](https://github.com/tree-sitter/tree-sitter/tree/master)

## Acknowledgement

1. [Sweep AI](https://github.com/sweepai/sweep) Sweep: AI-powered Junior Developer for small features and bug fixes.
2. [SourceGraph](https://github.com/sourcegraph/sourcegraph) Code AI platform with Code Search & Cody
3. [LLamaIndex](https://github.com/jerryjliu/llama_index) LlamaIndex (GPT Index) is a data framework for your LLM applications
4. [aider](https://github.com/paul-gauthier/aider) aider is AI pair programming in your terminal
    - [Improving GPT-4’s codebase understanding with ctags](https://aider.chat/docs/ctags.html)

## License

See [Elastic License 2.0].
