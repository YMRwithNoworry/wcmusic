import { useCallback, useEffect, useState } from "react";

import { sources } from "../api";
import type { SourceView } from "../types";

/// 音源页：内置音源 + 导入的 QuickJS 音源脚本。
export default function SourcesPage() {
  const [list, setList] = useState<SourceView[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setList(await sources.list());
    } catch (problem) {
      setError(String(problem));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const importFile = async () => {
    setError(null);
    setNotice(null);
    try {
      const path = await sources.pickFile();
      if (!path) {
        return;
      }
      setBusy(true);
      const imported = await sources.importFile(path);
      setNotice(`已导入并启用：${imported.name}`);
      await refresh();
    } catch (problem) {
      setError(String(problem));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="sources">
      <div className="section-head">
        <h1>音源</h1>
        <button onClick={() => void importFile()} disabled={busy}>
          {busy ? "正在校验…" : "导入音源文件"}
        </button>
      </div>

      {notice ? <p className="hint">{notice}</p> : null}
      {error ? <p className="error">{error}</p> : null}

      <ul className="source-list">
        {list.map((source) => (
          <li
            key={source.index === null ? "builtin" : `source-${source.index}`}
            className={source.active ? "source active" : "source"}
          >
            <div className="source-main">
              <div className="source-name">
                {source.name}
                {source.active ? <span className="badge">使用中</span> : null}
              </div>
              {source.capabilities.length > 0 ? (
                <div className="source-sub">支持：{source.capabilities.join("、")}</div>
              ) : null}
            </div>
            <div className="source-actions">
              {!source.active ? (
                <button
                  className="link"
                  onClick={() => {
                    void sources.select(source.index).then(setList).catch((problem) => {
                      setError(String(problem));
                    });
                  }}
                >
                  启用
                </button>
              ) : null}
              {source.index !== null ? (
                <button
                  className="link danger"
                  onClick={() => {
                    void sources
                      .remove(source.index as number)
                      .then(setList)
                      .catch((problem) => {
                        setError(String(problem));
                      });
                  }}
                >
                  删除
                </button>
              ) : null}
            </div>
          </li>
        ))}
      </ul>

      <p className="hint">
        音源脚本是第三方代码：运行时限制 32 MB 内存、单次网络请求 15 秒超时；导入的音源只保存在本次运行内。
      </p>
    </div>
  );
}
