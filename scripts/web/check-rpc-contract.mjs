import fs from "node:fs";
import path from "node:path";
import ts from "typescript";
import { loadSurface, repoRoot } from "./check-command-coverage.mjs";

const contract = JSON.parse(
  fs.readFileSync(
    path.join(repoRoot, "src/web-shims/command-contract.json"),
    "utf8",
  ),
);
const schemaFile = path.join(repoRoot, "src/lib/api/commandSchemas.ts");
const source = ts.createSourceFile(
  schemaFile,
  fs.readFileSync(schemaFile, "utf8"),
  ts.ScriptTarget.Latest,
  true,
);
const errors = [];
const declared = new Set();
function inspect(node) {
  if (
    ts.isVariableDeclaration(node) &&
    node.name.getText(source) === "rpcContracts" &&
    node.initializer &&
    ts.isObjectLiteralExpression(node.initializer)
  ) {
    for (const property of node.initializer.properties) {
      if (
        !ts.isPropertyAssignment(property) ||
        !ts.isObjectLiteralExpression(property.initializer)
      ) {
        errors.push(
          "RPC contracts must declare explicit argument and result schemas",
        );
        continue;
      }
      const name = property.name.getText(source).replace(/^['"]|['"]$/g, "");
      declared.add(name);
      const fields = property.initializer.properties.map((field) =>
        field.name?.getText(source),
      );
      if (!fields.includes("args") || !fields.includes("result"))
        errors.push(`${name}: missing argument or result schema`);
    }
  }
  ts.forEachChild(node, inspect);
}
inspect(source);
const expected = new Set(contract.validatedCommands);
if (!expected.size || expected.size !== contract.validatedCommands.length) {
  errors.push("validatedCommands must be non-empty and contain no duplicates");
}
const surface = loadSurface();
for (const name of expected) {
  if (!declared.has(name)) errors.push(`${name}: missing runtime schema`);
  if (!surface.handlers.has(name))
    errors.push(`${name}: missing server handler`);
  if (!surface.calls.has(name)) errors.push(`${name}: missing frontend call`);
}
for (const name of declared)
  if (!expected.has(name)) errors.push(`${name}: add to validatedCommands`);

const walk = (dir) =>
  fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) =>
      entry.isDirectory()
        ? walk(path.join(dir, entry.name))
        : [path.join(dir, entry.name)],
    );
const checked = new Set();
for (const file of walk(path.join(repoRoot, "src")).filter(
  (file) => /\.tsx?$/.test(file) && !/\.(test|spec)\.tsx?$/.test(file),
)) {
  const tree = ts.createSourceFile(
    file,
    fs.readFileSync(file, "utf8"),
    ts.ScriptTarget.Latest,
    true,
  );
  const rawNames = new Set();
  const typedNames = new Set();
  const namespaces = new Set();
  for (const node of tree.statements) {
    if (!ts.isImportDeclaration(node)) continue;
    const module = node.moduleSpecifier.text;
    const raw = module === "@tauri-apps/api/core";
    const typed = ["./checkedInvoke", "@/lib/api/checkedInvoke"].includes(
      module,
    );
    const bindings = node.importClause?.namedBindings;
    if (raw && bindings && ts.isNamespaceImport(bindings))
      namespaces.add(bindings.name.text);
    if (bindings && ts.isNamedImports(bindings)) {
      for (const binding of bindings.elements) {
        const name = (binding.propertyName ?? binding.name).text;
        if (raw && name === "invoke") rawNames.add(binding.name.text);
        if (typed && name === "invokeChecked")
          typedNames.add(binding.name.text);
      }
    }
  }
  function visit(node) {
    if (
      ts.isCallExpression(node) &&
      node.arguments[0] &&
      ts.isStringLiteralLike(node.arguments[0])
    ) {
      const name = node.arguments[0].text;
      const expression = node.expression;
      const typed =
        ts.isIdentifier(expression) && typedNames.has(expression.text);
      const raw =
        (ts.isIdentifier(expression) && rawNames.has(expression.text)) ||
        (ts.isPropertyAccessExpression(expression) &&
          ts.isIdentifier(expression.expression) &&
          namespaces.has(expression.expression.text) &&
          expression.name.text === "invoke");
      if (expected.has(name) && raw)
        errors.push(
          `${path.relative(repoRoot, file)}: use invokeChecked for ${name}`,
        );
      if (expected.has(name) && typed) checked.add(name);
    }
    ts.forEachChild(node, visit);
  }
  visit(tree);
}
for (const name of expected)
  if (!checked.has(name)) errors.push(`${name}: no typed API wrapper`);
if (errors.length) {
  console.error(errors.join("\n"));
  process.exitCode = 1;
} else {
  console.log(
    `Web RPC contracts OK: ${declared.size} commands with typed wrappers and runtime schemas.`,
  );
}
