// Enforces the layered-architecture rules (docs/layered-architecture.md §1, §8, §11.3) that were removed from
// atm-cli/build.py. Runs as part of npm run test:scripts.
//
// Rules:
//   - Every module lives in a layer's folder (common/, db/, vcs/, lib/, boot/, cli/, api/)
//   - Imports between modules follow the ALLOWED layer matrix
//   - cli/ and api/ modules never reach a project's store (.store / _store) directly
//   - A module only imports from earlier modules in MODULES order (build concatenation order)
//   - No two modules define the same top-level name

import fs from "node:fs";
import path from "node:path";

const ROOT = path.join(process.cwd(), "atm-cli", "atm");
const MODULES_ORDER = [
  "common.errors",
  "common.vocabularies",
  "common.core",
  "common.dashboard",
  "common.model",
  "common.fields",
  "common.types",
  "common.components",
  "common.sprint_names",
  "common.repository",
  "host.git",
  "db.interface",
  "vcs.interface",
  "db.jsongit.gitrun",
  "db.jsongit.files",
  "db.jsongit.paths",
  "db.jsongit.lock",
  "db.jsongit.worktrees",
  "db.jsongit.claims",
  "db.jsongit.logo",
  "db.jsongit.merge",
  "db.jsongit.migrate",
  "db.registry",
  "db.working",
  "db.jsongit.store",
  "db.memory",
  "vcs.git",
  "lib.project",
  "lib.state",
  "lib.loop",
  "lib.tasks",
  "lib.query",
  "lib.status",
  "lib.claims",
  "lib.edit",
  "lib.components",
  "lib.sprints",
  "lib.releases",
  "lib.archive",
  "lib.code",
  "lib.projects",
  "lib.settings",
  "lib.worktrees",
  "lib.documents",
  "boot.identity",
  "boot.open",
  "boot.jsongit",
  "cli.render",
  "cli.query",
  "cli.status",
  "cli.claims",
  "cli.edit",
  "cli.components",
  "cli.sprints",
  "cli.releases",
  "cli.archive",
  "cli.show",
  "cli.code",
  "cli.projects",
  "cli.settings",
  "cli.store",
  "cli.serve",
  "cli.main",
  "api.server",
];

// Which layers may import from which layers (docs/layered-architecture.md §1).
const ALLOWED = {
  common: ["common"],
  host: ["common", "host"],
  "db-interface": ["common", "db-interface"],
  db: ["common", "host", "db-interface", "db"],
  "vcs-interface": ["common", "vcs-interface"],
  vcs: ["common", "host", "vcs-interface", "vcs"],
  lib: ["common", "db-interface", "vcs-interface", "lib"],
  boot: ["common", "db-interface", "db", "vcs-interface", "vcs", "lib", "boot"],
  cli: ["common", "lib", "boot", "cli"],
  api: ["common", "lib", "boot", "api"],
};

function layerOf(module) {
  const parts = module.split(".");
  if (parts.length === 1) {
    throw new Error(`${module}.py is outside the layers: put it in a layer's folder`);
  }
  if (parts[0] === "db" || parts[0] === "vcs") {
    if (parts[1] === "interface") {
      return parts[0] + "-interface";
    }
  }
  if (!(parts[0] in ALLOWED)) {
    throw new Error(`${module}: ${parts[0]}/ isn't a layer's folder`);
  }
  return parts[0];
}

function resolve(importer, node) {
  const parts = importer.split(".");
  const packageParts = parts.slice(0, -1);
  const level = node.level || 1;
  if (level - 1 > packageParts.length) {
    throw new Error(`${importer}: an import goes above atm/`);
  }
  const base = packageParts.slice(0, packageParts.length - (level - 1));
  if (!node.module) {
    throw new Error(`${importer}: 'from . import x' between modules doesn't survive the build`);
  }
  return [...base, node.module].join(".");
}

function topLevelNames(source) {
  // Very lightweight Python AST-free approximation: look for top-level def/class/assign.
  // Good enough for catching duplicate names across modules.
  const names = new Set();
  const lines = source.split("\n");
  let indent = 0;
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const lineIndent = line.length - line.trimStart().length;
    if (lineIndent === 0) {
      if (trimmed.startsWith("def ") || trimmed.startsWith("class ")) {
        const m = trimmed.match(/^(?:def|class)\s+([A-Za-z_]\w*)/);
        if (m) names.add(m[1]);
      } else if (trimmed.startsWith("__all__")) {
        // skip
      }
    }
  }
  return names;
}

function findModules(dir) {
  const result = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      result.push(...findModules(full));
    } else if (entry.name.endsWith(".py") && entry.name !== "__init__.py") {
      const rel = path.relative(ROOT, full);
      const module = rel.replace(/\\/g, "/").replace(/\.py$/, "").replace(/\//g, ".");
      result.push({ module, path: full });
    }
  }
  return result;
}

const modules = findModules(ROOT);
const moduleMap = new Map(modules.map((m) => [m.module, m]));
const orderIndex = new Map(MODULES_ORDER.map((m, i) => [m, i]));

const errors = [];

// 1. Every module is in MODULES_ORDER
for (const { module } of modules) {
  if (!MODULES_ORDER.includes(module)) {
    errors.push(`${module}: not in MODULES_ORDER (build concatenation list)`);
  }
}

// 2. Every module in MODULES_ORDER exists on disk
for (const module of MODULES_ORDER) {
  if (!moduleMap.has(module)) {
    errors.push(`${module}: listed in MODULES_ORDER but no file on disk`);
  }
}

// Parse each module's imports
const importsByModule = new Map();
for (const { module, path: filePath } of modules) {
  const source = fs.readFileSync(filePath, "utf8");
  const names = new Set();

  // crude line-by-line import parsing
  const lines = source.split("\n");
  for (const line of lines) {
    const trimmed = line.trim();

    // import a.b.c
    const m1 = trimmed.match(/^import\s+([A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)/);
    if (m1) {
      names.add(m1[1]);
      continue;
    }

    // from X import Y  (absolute)
    const m2 = trimmed.match(/^from\s+([A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)\s+import\b/);
    if (m2) {
      names.add(m2[1]);
      continue;
    }

    // from .X import Y  (relative)
    const m3 = trimmed.match(/^from\s+(\.*)([A-Za-z_]\w*)\s+import\b/);
    if (m3) {
      const dots = m3[1].length;
      const mod = m3[2];
      const resolved = resolve(module, { level: dots, module: mod });
      names.add(resolved);
      continue;
    }
  }

  importsByModule.set(module, names);
}

// 3. Layer rules: each module's imports must be in ALLOWED[its layer]
for (const { module } of modules) {
  const layer = layerOf(module);
  const allowed = new Set(ALLOWED[layer]);
  const imported = importsByModule.get(module) || new Set();
  for (const target of imported) {
    if (!MODULES_ORDER.includes(target)) continue; // external import (click, etc.)
    const targetLayer = layerOf(target);
    if (!allowed.has(targetLayer)) {
      errors.push(
        `${module} (${layer}) imports from ${target} (${targetLayer}), which the ${layer} layer may not use`
      );
    }
  }
}

// 4. Import order: a module only imports from earlier modules in MODULES_ORDER
for (const { module } of modules) {
  const idx = orderIndex.get(module);
  if (idx === undefined) continue;
  const imported = importsByModule.get(module) || new Set();
  for (const target of imported) {
    if (!MODULES_ORDER.includes(target)) continue; // external import
    const tidx = orderIndex.get(target);
    if (tidx === undefined || tidx >= idx) {
      errors.push(
        `${module} imports from ${target}, which is not an earlier module in build order`
      );
    }
  }
}

// 5. cli/api modules never touch .store directly (§11.3)
for (const { module, path: filePath } of modules) {
  const layer = layerOf(module);
  if (layer !== "cli" && layer !== "api") continue;
  const source = fs.readFileSync(filePath, "utf8");
  const lines = source.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    // Skip import statements — importing cli/store module is fine
    if (trimmed.startsWith("import ") || trimmed.startsWith("from ")) continue;
    // Check for attribute access to .store or ._store (project's store, not cli/store module)
    if (/\.(store|_store)\b/.test(line)) {
      errors.push(
        `${module}:${i + 1} reaches a project's store (.store/_store): the ${layer} layer reads and changes data only through atm-lib`
      );
    }
  }
}

// 6. No duplicate top-level names across modules
const defined = new Map();
for (const { module, path: filePath } of modules) {
  const source = fs.readFileSync(filePath, "utf8");
  const names = topLevelNames(source);
  for (const name of names) {
    if (defined.has(name)) {
      errors.push(
        `${name} is defined in both ${module_file(defined.get(name))} and ${module_file(module)}`
      );
    }
    defined.set(name, module);
  }
}

function module_file(name) {
  return name.replace(/\./g, "/") + ".py";
}

if (errors.length > 0) {
  console.error("Architecture rule violations:");
  for (const e of errors) {
    console.error(`  - ${e}`);
  }
  process.exit(1);
}

console.log(`Architecture OK: ${modules.length} modules, 0 violations`);
