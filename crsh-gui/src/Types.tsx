export type PingResult = {
    success: boolean,
    time?: number,
    err?: any,
}

export function parseRemote(str: string): Remote | null {
    let sep = str.lastIndexOf(":");
    let addr = str.substring(0, sep);
    let port_str = str.substring(sep + 1);
    if (!addr || !port_str) {
        return null
    }
    let port = parseInt(port_str);
    return new Remote(addr, port)
}

export class Remote {
    address: string;
    port: number;

    constructor(addr: string, port: number) {
        this.address = addr;
        this.port = port;
    }

    display(): string {
        return this.address + ":" + this.port + "/";
    }

    async ping(): Promise<PingResult> {
        try {
            const t0 = Date.now();
            let resp = await fetch(this.display());
            console.log(resp);
            if (resp.ok) {
                const d = Date.now() - t0;
                return {
                    time: d,
                    success: true,
                }
            }
            return {
                success: false,
                err: resp.statusText
            }
        } catch (e) {
            return {
                success: false,
                err: e,
            }
        }
    }
}

export type HistLn = {
    message: string,
    stdtype: string
}

export type Command = {
    type: string,
    inner: string | IoCommand,
}

/**
 * Create  { dir: bool, path: String },
 * Delete  { dir: bool, path: String },
 * Display { path: String, bridge: String },
 * Write   { path: String, contents: String },
 * Append  { path: String, contents: String },
 * ListDir { path: String },
 */
export type IoCommand = {
    type: string,
    path: string,
    dir: boolean | undefined,
    contents: string | undefined,
    bridge: string | undefined,
}