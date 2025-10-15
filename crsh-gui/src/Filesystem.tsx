import {useEffect, useRef, useState} from "react";
import {invoke} from "@tauri-apps/api/core";

async function establish(token: string): Promise<string | undefined> {
    try {
        return await invoke("fs_est", {token});
    } catch (e) {
        console.error(e);
    }
}

async function query(token: string, bridge: string): Promise<FileSystemView | undefined> {
    try {
        return await invoke("fs_read", {token, bridge});
    } catch (e) {
        console.error(e);
    }
}

async function setFsPath(token: string, path: string) {
    try {
        let cmd = {
            type: 'Io',
            inner: {
                type: 'ListDir',
                path: path,
            }
        };
        await invoke("submit", {broadcast: false, cmd: cmd, token: token})
    } catch (e) {
        console.log(e)
    }
}

async function displayFile(token: string, bridge: string, path: string) {
    try {
        let cmd = {
            type: 'Io',
            inner: {
                type: 'Display',
                path: path,
                bridge: bridge
            }
        };
        await invoke("submit", {broadcast: false, cmd: cmd, token: token})
    } catch (e) {
        console.log(e)
    }
}

async function writeFile(token: string, path: string, contents: string[]) {
    try {
        let cmd = {
            type: 'Io',
            inner: {
                type: 'Write',
                path: path,
                contents: contents.join("")
            }
        };
        console.log(cmd.inner.contents)
        await invoke("submit", {broadcast: false, cmd: cmd, token: token})
    } catch (e) {
        console.log(e)
    }
}

async function deleteFile(token: string, path: string) {
    try {
        let cmd = {
            type: 'Io',
            inner: {
                type: 'Delete',
                dir: false,
                path: path,
            }
        };
        await invoke("submit", {broadcast: false, cmd: cmd, token: token})
    } catch (e) {
        console.log(e)
    }
}

type FileSystemView = {
    path: string,
    dir_info: FileEntry[],
    display: string
}

type FileEntry = {
    name: string,
    size: number,
}

const GB_SCALE = 1024 * 1024 * 1024;
const MB_SCALE = 1024 * 1024;
const KB_SCALE = 1024;

// super ugly but i dont care lol
function formatFileSize(size: number): string {
    if (size > GB_SCALE) {
        return `${(size / GB_SCALE).toPrecision(4)} GB`
    } else if (size > MB_SCALE) {
        return `${(size / MB_SCALE).toPrecision(4)} MB`
    } else if (size > KB_SCALE) {
        return `${(size / KB_SCALE).toPrecision(4)} KB`
    } else { // bytes
        return `${size} B`
    }
}

export default function Filesystem() {
    const [token, setToken] = useState<string | undefined>();

    const [id, setId] = useState<string | undefined>();
    const [fs, setFs] = useState<FileSystemView | undefined>();

    const pathRef = useRef<HTMLInputElement | null>(null);
    const [path, setPath] = useState<string>("");

    const [currentDisplay, setCurrentDisplay] = useState<string>("");
    const [selectedFile, setSelectedFile] = useState<string>("");
    const [editor, setEditor] = useState<string[]>([]);

    const pollRef = useRef<number | null>(null);
    useEffect(() => {
        const poll = () => {
            pollRef.current = setInterval(async () => {
                if (token && id) {
                    if (currentDisplay != selectedFile) {
                        setCurrentDisplay(selectedFile);
                        if (fs) {
                            setEditor(fs.display.split("\n"))
                            setFs({
                                path: fs.path,
                                dir_info: fs.dir_info,
                                display: ""
                            })
                        }
                        await displayFile(token, id, selectedFile)
                    }
                    setFs(await query(token, id))
                    if (document.activeElement != pathRef.current && fs) {
                        setPath(fs.path)
                    }
                }
            }, 800);
        };
        poll();

        return () => {
            clearInterval(pollRef.current as number);
        }
    }, [token, id, selectedFile, currentDisplay]);

    if (!fs || !id) {
        return (
            <>
                <div className="w-full">
                    <span className="font-semibold text-2xl text-center">Establish Filesystem Bridge</span>
                    <div className="flex mt-3 m-auto">
                        <input className="min-w-72 mr-1 bg-slate-900 p-2 pl-3 rounded-lg m-auto"
                               type="text" placeholder="Token..."
                               onChange={e => setToken(e.target.value)}
                        />
                        <button className="mr-auto font-semibold tracking-widest ml-1 rounded-lg pl-4 pr-4
                                        transition bg-blue-700 cursor-pointer
                                         hover:bg-blue-800 active:bg-blue-900"
                                onClick={async () => {
                                    let id = await establish(token as string);
                                    if (id) {
                                        console.log("Set FS ID: " + id);
                                        setId(id);
                                    }
                                }
                                }>
                            Link
                        </button>
                    </div>
                </div>
            </>
        )
    } else {
        return (
            <>
                <div className="w-full">
                    <div className="font-semibold text-2xl text-center">Filesystem</div>
                    <div className="font-thin text-zinc-300 text-center">Bridge ID: {id}</div>
                    <div className="mt-2 w-full">
                        <div className="p-2 bg-zinc-950 rounded-md w-full">
                            <div className="pb-4">
                                <div className="w-full flex">
                                    <span
                                        className="font-normal m-auto ml-2 mr-4 text-slate-300 align-middle">Path</span>
                                    <form className="w-full"
                                          onSubmit={async e => {
                                              e.preventDefault();
                                              await setFsPath(token as string, path as string)
                                          }}
                                    >
                                        <input
                                            ref={pathRef}
                                            className="bg-slate-600 w-full pl-2 rounded-md pt-1 pb-1"
                                            placeholder="No path selected."
                                            value={path}
                                            onChange={e => setPath(e.target.value)}
                                        />
                                    </form>
                                </div>
                                <span className="flex">
                                <button></button>
                            </span>
                            </div>
                            <ol className="ml-1 h-128 overflow-y-scroll">
                                {fs.dir_info.sort((a, b) => {
                                    if (a.name.toLowerCase() > b.name.toLowerCase()) return 1
                                    return -1
                                }).map((entry) => {
                                    return (
                                        <>
                                            <li>
                                                <button className="pl-1 flex w-full hover:bg-slate-600"
                                                        onClick={() => setSelectedFile(entry.name)}>
                                                    <span className="mr-auto">{entry.name}</span>
                                                    <span className="mr-8">{formatFileSize(entry.size)}</span>
                                                </button>
                                            </li>
                                        </>
                                    )
                                })}
                            </ol>
                        </div>
                        {currentDisplay &&
                            <div className="mt-5">
                                <div className="text-center font-normal text-lg">
                                    <span>File Details - </span>
                                    <span className="text-center font-light underline underline-offset-2">
                                        {currentDisplay}</span>
                                </div>
                                <div className="bg-zinc-900 p-2 rounded-md">
                                    <ol className="h-128 overflow-y-scroll">
                                        {fs.display.split("\n").map((line, i) => {
                                            return (
                                                <>
                                                    <li className="flex">
                                                        <span
                                                            className="mr-2 min-w-6 max-w-6 font-normal text-slate-400">{i}</span>
                                                        <input
                                                            className="w-full select-none"
                                                            defaultValue={line}
                                                            onChange={(e) => {
                                                                const n = editor.map((s, j) => {
                                                                    if (j === i) {
                                                                        return e.target.value
                                                                    } else {
                                                                        return s
                                                                    }
                                                                });
                                                                setEditor(n)
                                                            }}
                                                        />
                                                    </li>
                                                </>
                                            )
                                        })}
                                    </ol>
                                    <div className="mt-2 border-zinc-950 border-3 rounded-lg pb-3">
                                        <div className="m-auto mt-3 mb-4 bg-zinc-700 h-0.5 w-11/12"></div>
                                        <div className="ml-6">
                                            <button className="ml-auto transition p-2 pr-11 pl-11 tracking-widest font-normal
                                            bg-green-700 hover:bg-green-800 active:bg-green-900 mr-3
                                            rounded-lg" onClick={async () => {
                                                await writeFile(token as string, selectedFile, editor)
                                            }}>
                                                Write
                                            </button>
                                            <button className="ml-auto transition p-2 pr-11 pl-11 tracking-widest font-normal
                                            bg-red-700 hover:bg-red-800 active:bg-red-900 mr-3
                                            rounded-lg" onClick={async () => {
                                                await deleteFile(token as string, selectedFile)
                                            }}>
                                                Delete
                                            </button>
                                            <button className="ml-auto transition p-2 pr-5 pl-5 tracking-widest font-normal
                                            bg-yellow-600 hover:bg-yellow-700 active:bg-yellow-800 mr-3
                                            rounded-lg" onClick={async () => {
                                                await displayFile(token as string, id, selectedFile)
                                            }}>
                                                Refresh
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }
                    </div>
                </div>

            </>
        )
    }
}
