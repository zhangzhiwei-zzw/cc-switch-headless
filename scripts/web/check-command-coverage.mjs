import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

export const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const walk = (dir) =>
  fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) =>
      entry.isDirectory()
        ? walk(path.join(dir, entry.name))
        : [path.join(dir, entry.name)],
    );

export function collectInvokes(text, file) {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const aliases = new Set();
  const namespaces = new Set();
  const commands = [];
  const dynamic = [];
  for (const statement of source.statements) {
    if (
      !ts.isImportDeclaration(statement) ||
      statement.moduleSpecifier.text !== "@tauri-apps/api/core"
    )
      continue;
    const bindings = statement.importClause?.namedBindings;
    if (bindings && ts.isNamespaceImport(bindings))
      namespaces.add(bindings.name.text);
    if (bindings && ts.isNamedImports(bindings)) {
      for (const entry of bindings.elements)
        if ((entry.propertyName ?? entry.name).text === "invoke")
          aliases.add(entry.name.text);
    }
  }
  function visit(node) {
    if (ts.isCallExpression(node)) {
      const expression = node.expression;
      const invoke =
        (ts.isIdentifier(expression) && aliases.has(expression.text)) ||
        (ts.isPropertyAccessExpression(expression) &&
          ts.isIdentifier(expression.expression) &&
          namespaces.has(expression.expression.text) &&
          expression.name.text === "invoke");
      if (invoke) {
        const cmd = node.arguments[0];
        if (
          cmd &&
          (ts.isStringLiteral(cmd) || ts.isNoSubstitutionTemplateLiteral(cmd))
        )
          commands.push(cmd.text);
        else
          dynamic.push(
            `${file}: dynamic invoke requires explicit review (${cmd?.getText(source) ?? "missing argument"})`,
          );
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  return { commands, dynamic };
}

export function loadSurface(root = repoRoot) {
  const routes = fs.readFileSync(
    path.join(root, "src-tauri/src/web/routes.rs"),
    "utf8",
  );
  const handlers = new Set(
    [...routes.matchAll(/table\.insert\(\s*"(\w+)"/g)].map((m) => m[1]),
  );
  for (const [, module] of routes.matchAll(/super::(\w+)::HANDLERS/g)) {
    const source = fs.readFileSync(
      path.join(root, `src-tauri/src/web/${module}.rs`),
      "utf8",
    );
    const table = source.match(
      /pub const HANDLERS:[\s\S]*?=\s*&\[([\s\S]*?)\n\];/,
    );
    if (!table) throw new Error(`Cannot parse ${module}::HANDLERS`);
    for (const [, cmd] of table[1].matchAll(/\(\s*"(\w+)"\s*,/g))
      handlers.add(cmd);
  }
  const calls = new Map();
  const dynamic = [];
  for (const file of walk(path.join(root, "src")).filter(
    (file) =>
      /\.[jt]sx?$/.test(file) &&
      !/\.(test|spec)\.[jt]sx?$/.test(file) &&
      !file.includes(`${path.sep}tray-panel${path.sep}`) &&
      !file.includes(`${path.sep}web-shims${path.sep}`),
  )) {
    const name = path.relative(root, file).replaceAll(path.sep, "/");
    const found = collectInvokes(fs.readFileSync(file, "utf8"), name);
    dynamic.push(...found.dynamic);
    for (const cmd of found.commands)
      calls.set(cmd, [...(calls.get(cmd) ?? []), name]);
  }

  const shim = fs.readFileSync(
    path.join(root, "src/web-shims/core.ts"),
    "utf8",
  );
  const browser = new Set(
    [...shim.matchAll(/case "(\w+)":/g)].map((m) => m[1]),
  );
  return { handlers, calls, browser, dynamic };
}

export function checkCoverage(
  contract,
  { handlers, calls, browser, dynamic = [] },
) {
  const errors = [...dynamic];
  for (const [name, feature] of Object.entries(contract.features)) {
    if (feature.enabled === false) continue;
    const appCommands = Object.values(contract.apps)
      .filter((app) => app.feature === name)
      .flatMap((app) => app.commands);
    for (const cmd of [...feature.commands, ...appCommands]) {
      if (!handlers.has(cmd))
        errors.push(`${name}: missing server command ${cmd}`);
    }
  }
  for (const [app, spec] of Object.entries(contract.apps)) {
    if (!contract.features[spec.feature])
      errors.push(`${app}: unknown feature ${spec.feature}`);
    for (const cmd of spec.commands)
      if (!calls.has(cmd))
        errors.push(`${app}: stale frontend requirement ${cmd}`);
  }
  for (const cmd of contract.browserCommands)
    if (!browser.has(cmd)) errors.push(`Missing browser adapter: ${cmd}`);
  for (const [cmd, files] of calls) {
    if (!handlers.has(cmd) && !browser.has(cmd) && !contract.unsupported[cmd])
      errors.push(`Unclassified frontend command ${cmd}: ${files.join(", ")}`);
  }
  for (const [cmd, reason] of Object.entries(contract.unsupported)) {
    if (!reason.trim()) errors.push(`Missing explanation for ${cmd}`);
    if (!calls.has(cmd)) errors.push(`Stale unsupported command ${cmd}`);
    if (handlers.has(cmd) || browser.has(cmd))
      errors.push(
        `Command ${cmd} is implemented; remove its unsupported entry`,
      );
  }
  return errors;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const contract = JSON.parse(
    fs.readFileSync(
      path.join(repoRoot, "src/web-shims/command-contract.json"),
      "utf8",
    ),
  );
  const surface = loadSurface();
  const errors = checkCoverage(contract, surface);
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exitCode = 1;
  } else {
    console.log(
      `Web command coverage OK: ${surface.calls.size} frontend commands reviewed, ${surface.handlers.size} server commands, ${Object.keys(contract.apps).length} apps.`,
    );
  }
}
