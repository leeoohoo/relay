import { useEffect, useState } from "react";

import { API_BASE_URL } from "../api/client";
import type { Message, MessageAttachment } from "../types/chat";
import { Icon } from "./ui";

function messageAttachmentUrl(message: Message, attachment: MessageAttachment) {
  return `${API_BASE_URL.replace(/\/$/u, "")}/api/v1/conversations/${message.conversation_id}/messages/${message.id}/attachments/${attachment.id}`;
}

function MessageAttachmentImage(props: {
  message: Message;
  attachment: MessageAttachment;
  token: string;
  onError: (error: unknown) => void;
}) {
  const [source, setSource] = useState("");
  useEffect(() => {
    let active = true;
    let objectUrl = "";
    void fetch(messageAttachmentUrl(props.message, props.attachment), {
      headers: { authorization: `Bearer ${props.token}` },
    })
      .then(async (response) => {
        if (!response.ok) throw new Error(`图片加载失败 (${response.status})`);
        return response.blob();
      })
      .then((blob) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(blob);
        setSource(objectUrl);
      })
      .catch((error) => { if (active) props.onError(error); });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [props.message.id, props.attachment.id, props.token]);
  return source
    ? <img src={source} alt={props.attachment.file_name} />
    : <span className="attachment-image-loading">图片加载中…</span>;
}

async function downloadMessageAttachment(
  message: Message,
  attachment: MessageAttachment,
  token: string,
) {
  const response = await fetch(messageAttachmentUrl(message, attachment), {
    headers: { authorization: `Bearer ${token}` },
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? `附件下载失败 (${response.status})`);
  }
  const objectUrl = URL.createObjectURL(await response.blob());
  const anchor = document.createElement("a");
  anchor.href = objectUrl;
  anchor.download = attachment.file_name;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(objectUrl), 1_000);
}

function formatByteSize(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / 1024 / 1024).toFixed(1)} MiB`;
}

export function MessageAttachments(props: {
  message: Message;
  token: string;
  onError: (error: unknown) => void;
}) {
  const attachments = props.message.attachments ?? [];
  if (!attachments.length) return null;
  return (
    <div className="message-attachments">
      {attachments.map((attachment) => {
        if (attachment.kind === "local_folder") {
          return (
            <details className="message-folder-reference" key={attachment.id}>
              <summary><span className="attachment-kind-icon"><Icon name="folder" /></span><span><strong>{attachment.file_name}</strong><small>{attachment.local_path}</small></span><span>{attachment.directory_entries.length} 项</span></summary>
              <div><p>用途：作为新项目导入，并由 Agent 推送到项目 Git。</p><pre>{attachment.directory_entries.slice(0, 300).join("\n") || "（空目录）"}{attachment.directory_entries.length > 300 ? `\n… 另有 ${attachment.directory_entries.length - 300} 项` : ""}</pre></div>
            </details>
          );
        }
        return (
          <div className={`message-file-attachment ${attachment.kind}`} key={attachment.id}>
            {attachment.kind === "image"
              ? <button className="message-image-preview" type="button" title="下载原图" onClick={() => void downloadMessageAttachment(props.message, attachment, props.token).catch(props.onError)}><MessageAttachmentImage message={props.message} attachment={attachment} token={props.token} onError={props.onError} /></button>
              : <span className="attachment-kind-icon"><Icon name="paperclip" /></span>}
            <span><strong>{attachment.relative_path || attachment.file_name}</strong><small>{formatByteSize(attachment.byte_size)} · {attachment.content_type}</small></span>
            <button className="attachment-download" type="button" onClick={() => void downloadMessageAttachment(props.message, attachment, props.token).catch(props.onError)}><Icon name="download" /> 下载</button>
          </div>
        );
      })}
    </div>
  );
}
