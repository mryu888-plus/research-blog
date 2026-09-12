'use strict';
const { createConnection, ProposedFeatures, TextDocuments, TextDocumentSyncKind } = require('vscode-languageserver/node');
const { TextDocument } = require('vscode-languageserver-textdocument');
const { BlogWorkspace } = require('./engine.cjs');
const connection = createConnection(ProposedFeatures.all);
const documents = new TextDocuments(TextDocument);
let workspace, timer;
const published = new Set();
function publish() {
  clearTimeout(timer);
  for (const uri of published) if (!workspace.articles.has(uri)) { connection.sendDiagnostics({ uri, diagnostics: [] }); published.delete(uri); }
  for (const [uri, article] of workspace.articles) {
    connection.sendDiagnostics({ uri, version: workspace.open.has(uri) ? article.doc.version : undefined, diagnostics: workspace.diagnostics(uri) });
    published.add(uri);
  }
}
function schedule() { clearTimeout(timer); timer = setTimeout(publish, 100); }
connection.onInitialize(params => {
  if (!params.initializationOptions?.contentRoot) throw new Error('initializationOptions.contentRoot is required');
  workspace = new BlogWorkspace(params.initializationOptions.contentRoot);
  workspace.scan();
  return { capabilities: {
    textDocumentSync: TextDocumentSyncKind.Incremental,
    completionProvider: { triggerCharacters: ['[', '#', '"', '=', '/'] },
    definitionProvider: true, hoverProvider: true, referencesProvider: true, documentSymbolProvider: true,
  }, serverInfo: { name: 'research-blog-lsp', version: '0.1.0' } };
});
connection.onInitialized(schedule);
documents.onDidOpen(({ document }) => { if (workspace.accepts(document.uri)) workspace.open.add(document.uri); });
documents.onDidChangeContent(({ document }) => { workspace.update(document.uri, document.getText(), document.version); schedule(); });
documents.onDidClose(({ document }) => { workspace.open.delete(document.uri); workspace.scan(); schedule(); });
connection.onDidChangeWatchedFiles(() => { workspace.scan(); schedule(); });
connection.onCompletion(params => workspace.completion(params.textDocument.uri, params.position));
connection.onDefinition(params => workspace.definition(params.textDocument.uri, params.position));
connection.onHover(params => workspace.hover(params.textDocument.uri, params.position));
connection.onReferences(params => workspace.references(params.textDocument.uri, params.position, params.context.includeDeclaration));
connection.onDocumentSymbol(params => workspace.symbols(params.textDocument.uri));
connection.onShutdown(() => clearTimeout(timer));
documents.listen(connection);
connection.listen();
