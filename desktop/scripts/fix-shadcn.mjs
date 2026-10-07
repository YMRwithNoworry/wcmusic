// 一次性脚本：把注册表源码里的占位符换成项目实际用的东西。
//
// 1) `@/registry/radix-lyra/ui/*` → `@/components/ui/*`
// 2) `<IconPlaceholder phosphor="XIcon" ... />` → `<XIcon />`，并从 @phosphor-icons/react 导入
// 3) sonner.tsx 不再依赖 next-themes（Vite 应用里主题来自我们自己的设置）
//    注意：sonner 会 import 本项目的 settings-context，所以改完这一步不要覆盖回 theme="dark"。
import fs from "node:fs";
import path from "node:path";

const dir = path.resolve(process.cwd(), "src/components/ui");

for (const name of fs.readdirSync(dir)) {
  if (!name.endsWith(".tsx")) continue;
  const file = path.join(dir, name);
  let source = fs.readFileSync(file, "utf8");

  source = source.replaceAll("@/registry/radix-lyra/ui/", "@/components/ui/");

  const icons = new Set();
  source = source.replace(/<IconPlaceholder\b([\s\S]*?)\/>/g, (_match, attributes) => {
    const phosphor = /phosphor="([^"]+)"/.exec(attributes)?.[1];
    const className = /className="([^"]*)"/.exec(attributes)?.[1];
    if (!phosphor) return "";
    icons.add(phosphor);
    return className ? `<${phosphor} className="${className}" />` : `<${phosphor} />`;
  });

  if (icons.size > 0) {
    source = source.replace(
      /import\s*\{\s*IconPlaceholder\s*\}\s*from\s*"@\/app\/\(create\)\/components\/icon-placeholder"\n?/,
      "",
    );
    const list = [...icons].sort().join(", ");
    source = `import { ${list} } from "@phosphor-icons/react"\n${source}`;
  }

  if (name === "sonner.tsx") {
    source = source.replace(/import \{ useTheme \} from "next-themes"\n?/, 'import { useSettings } from "@/settings-context"\n');
    source = source.replace(
      /\s*const \{ theme = "system" \} = useTheme\(\)\n?/,
      "\n  const { settings } = useSettings()\n",
    );
    source = source.replace(
      /theme=\{theme as ToasterProps\["theme"\]\}/,
      'theme={settings?.dark_theme === false ? "light" : "dark"}',
    );
  }

  fs.writeFileSync(file, source);
  console.log("处理", name, icons.size > 0 ? `（图标：${[...icons].join(" ")}）` : "");
}
