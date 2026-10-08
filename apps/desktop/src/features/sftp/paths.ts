/** Remote paths are always POSIX, whatever the local OS is. */

export function joinPath(dir: string, name: string): string {
  return dir.endsWith("/") ? `${dir}${name}` : `${dir}/${name}`;
}

export function parentPath(path: string): string {
  const trimmed = path.length > 1 ? path.replace(/\/+$/, "") : path;
  const i = trimmed.lastIndexOf("/");
  if (i <= 0) return "/";
  return trimmed.slice(0, i);
}

/** Splits "/srv/api" into ["/srv/", "api"] so the last segment can be emphasised. */
export function splitLastSegment(path: string): [string, string] {
  if (path === "/") return ["", "/"];
  const trimmed = path.replace(/\/+$/, "");
  const i = trimmed.lastIndexOf("/");
  return [trimmed.slice(0, i + 1), trimmed.slice(i + 1)];
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}
