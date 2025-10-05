import {useEffect, useRef, useState} from "react";
import "./App.css";
import {HistLn, parseRemote, Remote} from "./Types.tsx";
import {invoke} from "@tauri-apps/api/core";

function App() {
    const [remote, setRemote] = useState<Remote>();

    // -1 = fail, -2 = no op, >0 = ms
    const [pingMs, setPingMs] = useState(-2);
    // const [prompt, setPrompt] = useState("");

    const pollRef = useRef<number | null>(null);
    const [history, setHistory] = useState<HistLn[]>([]);

    useEffect(() => {
        const poll = () => {
            pollRef.current = setInterval(async () => {
                try {
                    setHistory(await invoke("query"));
                } catch (e) {
                    setHistory([e as HistLn])
                }
            }, 800);
        };
        poll();

        return () => {
            clearInterval(pollRef.current as number);
        }
    }, []);

    async function ping() {
        if (remote) {
            const resp = await remote.ping();
            if (resp.success) {
                setPingMs(resp.time as number);
                await invoke("set_remote", {remote: remote.display()})
                return
            }
        }
        setPingMs(-1)
    }

    return (
        <main>
            <div className="bg-slate-800">
                <div className="p-8 text-center">
                    <h1 className="text-6xl text-zinc-100 font-extrabold select-none cursor-default">CRSH</h1>
                    <h2 className="text-2xl text-zinc-400 italic select-none cursor-default">
                        <b>C</b>entralised <b>R</b>emote <b>SH</b>ell
                    </h2>
                </div>

                <div className="mt-8">
                    <div className="text-center text-zinc-300 select-none cursor-default">Set remote router</div>
                    <div className="flex flex-col items-center">
                        {pingMs == -1 &&
                            <div>
                        <span
                            className="text-sm text-red-700">Failed to ping remote: it is either offline or misconfigured.</span>
                            </div>
                        }
                        {pingMs >= 0 &&
                            <div>
                                <span className="text-sm text-sky-700">Remote is up: connected in {pingMs} ms.</span>
                            </div>
                        }
                        <form className="flex items-center"
                              onSubmit={async e => {
                                  e.preventDefault();
                                  await ping();
                              }}>
                            <input
                                className="w-72 h-10 m-4 mr-1 p-2 pl-4 pr-4 bg-neutral-800 rounded-xl placeholder-zinc-400 text-zinc-200"
                                type="text" placeholder="address:port"
                                onChange={e => {
                                    setRemote(parseRemote(e.target.value))
                                }}/>
                            <button
                                className="cursor-pointer pl-2 pr-2 h-10 rounded-xl text-zinc-200
                            bg-teal-600 active:bg-teal-800 hover:bg-teal-700 transition"
                                type="submit">Bind
                            </button>
                        </form>
                    </div>
                </div>

                <div className="flex flex-col items-center mt-2 font-light text-sm pb-6">
                    <div className="w-4/5">
                        <div
                            className="h-128 md:h-180 lg:h-200 xl:h-256 bg-gray-900 border-slate-950 border-4 rounded-xl flex flex-col justify-between">
                            <ol className="p-2 pt-1 overflow-y-scroll flex flex-col h-full w-full ">
                                {history.map((ln) => {
                                    switch (ln.stdtype) {
                                        case "Out":
                                            return <span className="text-stone-300">{ln.message}</span>
                                        case "Err":
                                            return <span className="text-red-600">{ln.message}</span>
                                        default:
                                            return <span>?? {ln.message}</span>
                                    }
                                })}
                            </ol>
                            <div
                                className="rounded-b-xl bg-zinc-900 border-t-3 border-slate-950">
                                <form className="flex w-full justify-between"
                                      onSubmit={e => {
                                          e.preventDefault();

                                      }}>
                                    <input className="p-2 w-full" placeholder="Command..."/>
                                    <button
                                        className="p-2 pr-3 pl-3 bg-zinc-950 cursor-pointer active:bg-slate-950 hover:bg-stone-950 transition"
                                        type="submit">
                                        Submit
                                    </button>
                                </form>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
            <span className="text-sm text-center">Author: HerrPhoenix</span>
        </main>
    );
}

export default App;
