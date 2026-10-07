// 一次性脚本：从 shadcn 注册表拉取组件源码直接落盘。
//
// 为什么不用 `shadcn add`：CLI 会在装依赖时调用 `npm install --allow-scripts`，
// 而本机 npm 11 的全局 `allow-scripts` 配置在项目内安装时会被拒绝（EALLOWSCRIPTS），
// 导致组件文件根本写不出来。这里只取源码，依赖自己装。
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const components = process.argv.slice(2);
const root = path.resolve(process.cwd());
const seen = new Set();
const deps = new Set();

function targetFor(registryPath) {
  const parts = registryPath.split("/");
  const file = parts.at(-1);
  if (parts.includes("ui")) return path.join(root, "src/components/ui", file);
  if (parts.includes("lib")) return path.join(root, "src/lib", file);
  if (parts.includes("hooks")) return path.join(root, "src/hooks", file);
  return path.join(root, "src", file);
}

function fetch(name) {
  if (seen.has(name)) return;
  seen.add(name);
  const raw = execFileSync("npx", ["--yes", "shadcn@latest", "view", name], {
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    shell: true,
  });
  const items = JSON.parse(raw.slice(raw.indexOf("[")));
  for (const item of items) {
    for (const dep of item.dependencies ?? []) {
      if (dep !== "cn") deps.add(dep);
    }
    for (const file of item.files ?? []) {
      const target = targetFor(file.path);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      const content = file.content.replaceAll('from "cn"', 'from "@/lib/utils"');
      fs.writeFileSync(target, content);
      console.log("写入", path.relative(root, target));
    }
    for (const dependency of item.registryDependencies ?? []) {
      fetch(String(dependency).replace(/^@shadcn\//, ""));
    }
  }
}

for (const component of components) fetch(component);
if (deps.size > 0) console.log("\n需要安装的依赖：", [...deps].join(" "));
