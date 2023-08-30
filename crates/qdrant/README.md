# Qdrant Integration into CodeIndex

**Qdrant** (read: quadrant) is a vector similarity search engine and vector database. It provides a production-ready service with a convenient API to store, search, and manage points—vectors with an additional payload Qdrant is tailored to extended filtering support. It makes it useful for all sorts of neural-network or semantic-based matching, faceted search, and other applications.


> [!IMPORTANT]  
> We respect qdrant's [Apache License 2.0](./LICENSE) and strictly follow its rules. If you're looking for a Vector Database solution, please check [Qdrant Website](https://qdrant.tech/).

We've directly integrated Qdrant into CodeIndex project to achieve:

1. **Performance Improvement:** Direct library function calls within the same process are faster than network requests, even on the same machine. This reduces latency and optimizes application speed.

2. **Direct API Calls:** This allows greater flexibility with direct access to all Qdrant functionalities without network interface restrictions. It simplifies our codebase, promotes safer code with Rust's static typing, and eliminates the need for network communication code for Qdrant interaction.