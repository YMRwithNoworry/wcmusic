import { PlugsConnectedIcon, TrashIcon, UploadSimpleIcon } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { sources } from "@/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { SourceView } from "@/types";

/// 音源页：内置音源 + 导入的 QuickJS 音源脚本。
export default function SourcesPage() {
  const [list, setList] = useState<SourceView[] | null>(null);
  const [busy, setBusy] = useState(false);
  const fileInput = useRef<HTMLInputElement | null>(null);

  const refresh = useCallback(async () => {
    try {
      setList(await sources.list());
    } catch (problem) {
      toast.error(String(problem));
      setList([]);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  /// 用隐藏的 file input 选文件：WebView2 自带的文件选择器可靠，
  /// 后端原生对话框（rfd）在这个进程里根本弹不出来。
  const onPickFile = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    // 清空 value，才能重复选同一个文件。
    event.target.value = "";
    if (!file) {
      return;
    }
    setBusy(true);
    try {
      const script = await file.text();
      const imported = await sources.importScript(file.name, script);
      toast.success(`已导入并启用：${imported.name}`);
      await refresh();
    } catch (problem) {
      toast.error(String(problem));
    } finally {
      setBusy(false);
    }
  };

  const select = async (index: number | null) => {
    try {
      setList(await sources.select(index));
    } catch (problem) {
      toast.error(String(problem));
    }
  };

  const remove = async (index: number) => {
    try {
      setList(await sources.remove(index));
      toast.success("已删除该音源");
    } catch (problem) {
      toast.error(String(problem));
    }
  };

  return (
    <div className="h-full overflow-y-auto px-7 py-6">
      <header className="mb-5 flex items-center justify-between gap-4">
        <div>
          <h1 className="text-[22px] font-semibold">音源</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            音源脚本是第三方代码：运行时限制 32 MB 内存、单次请求 15 秒超时；导入的音源只在本次运行内有效。
          </p>
        </div>
        <Button onClick={() => fileInput.current?.click()} disabled={busy}>
          <UploadSimpleIcon weight="bold" />
          {busy ? "正在校验" : "导入音源文件"}
        </Button>
        <input
          ref={fileInput}
          type="file"
          accept=".js,text/javascript,application/javascript"
          className="hidden"
          onChange={(event) => void onPickFile(event)}
        />
      </header>

      {list === null ? (
        <div className="flex flex-col gap-2">
          {Array.from({ length: 2 }).map((_, index) => (
            <Skeleton key={index} className="h-16 rounded-xl" />
          ))}
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {list.map((source) => (
            <Card
              key={source.index === null ? "builtin" : `source-${source.index}`}
              className={`flex flex-row items-center justify-between gap-4 px-4 py-3 ${
                source.active ? "border-primary/50 bg-accent/40" : ""
              }`}
            >
              <div className="flex min-w-0 items-center gap-3">
                <PlugsConnectedIcon
                  weight={source.active ? "fill" : "regular"}
                  className={source.active ? "size-5 text-primary" : "size-5 text-muted-foreground"}
                />
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium">{source.name}</span>
                    {source.active ? <Badge>使用中</Badge> : null}
                  </div>
                  {source.capabilities.length > 0 ? (
                    <div className="truncate text-xs text-muted-foreground">
                      支持：{source.capabilities.join("、")}
                    </div>
                  ) : null}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                {!source.active ? (
                  <Button variant="ghost" size="sm" onClick={() => void select(source.index)}>
                    启用
                  </Button>
                ) : null}
                {source.index !== null ? (
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    className="text-muted-foreground hover:text-destructive"
                    title="删除"
                    onClick={() => void remove(source.index as number)}
                  >
                    <TrashIcon />
                  </Button>
                ) : null}
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
