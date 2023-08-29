# CodeIndex

CodeIndex is a local-first high performance codebase index engine designed for AI. It helps your LLM understanding the structure and semantics of a codebase and grabs code context based on the inputs.

## Features

- **🔐 Privacy first:** All data is stored locally on your machine.
- **🚀 High performance:** Indexing a normal codebase takes in seconds.
- **🤖 AI ready:** The index is designed for AI, which means it's easy to provide codebase context to your LLM.
- **⏰ Real-time:** The index is designed to be real-time. It can update indexes while you are typing.
- **⚙️ Configurable:** The index is designed to be configurable. You can customize the engine to fit your needs.

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
