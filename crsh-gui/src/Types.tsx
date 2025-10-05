export type PingResult = {
    success: boolean,
    time?: number,
}

export function parseRemote(str: string): Remote {
    let sep = str.lastIndexOf(":");
    let addr = str.substring(0, sep);
    let port = parseInt(str.substring(sep + 1));
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
        return "http://" + this.address + ":" + this.port + "/";
    }

    async ping(): Promise<PingResult> {
        try {
            const t0 = Date.now();
            await fetch(this.display());
            const d = Date.now() - t0;
            return {
                time: d,
                success: true
            }
        } catch (e) {
            return {
                time: undefined,
                success: false
            }
        }
    }
}

export type HistLn = {
    message: string,
    stdtype: string
}