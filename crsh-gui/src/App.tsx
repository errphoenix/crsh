import {useEffect, useRef, useState} from "react";
import "./App.css";
import {HistLn, parseRemote, Remote} from "./Types.tsx";
import {invoke} from "@tauri-apps/api/core";

function App() {
    const [remote, setRemote] = useState<Remote | null>();

    // -1 = fail, -2 = no op, >0 = ms
    const [pingMs, setPingMs] = useState(-2);
    const [pingErr, setPingErr] = useState<string>("");

    const pollRef = useRef<number | null>(null);
    const [history, setHistory] = useState<HistLn[]>([]);

    const [prompt, setPrompt] = useState<string>("");
    const [submitting, setSubmitting] = useState<boolean>(false);
    const formRef = useRef<HTMLFormElement>(null);

    const [resetTarget, setResetTarget] = useState<string>("");

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
            } else {
                console.log(`${resp.err}`);
                setPingErr(`${resp.err}`);
            }
        } else {
            setPingErr("invalid remote");
        }
        setPingMs(-1)
    }

    async function reset() {
        if (!remote) {
            setHistory([
                ...history,
                {
                    message: "No remote set.",
                    stdtype: 'Err'
                }
            ])
            return;
        }
        try {
            await invoke("reset", {token: resetTarget});
            setHistory([
                ...history,
                {
                    message: "Sent reset request to remote target.",
                    stdtype: 'Out'
                }
            ])
        } catch (e) {
            setHistory([
                ...history,
                {
                    message: "Failed to send reset request: " + e,
                    stdtype: 'Err'
                }
            ])
        }
    }

    async function submit(e: React.FormEvent) {
        e.preventDefault();
        const cmd = prompt.trim();
        if (submitting || !cmd) {
            return
        }

        setSubmitting(true);
        try {
            await invoke("submit", {broadcast: true, cmd: cmd});
            setPrompt("");
            formRef.current?.reset();
        } catch (e) {
            setHistory([
                ...history,
                {
                    message: e as string,
                    stdtype: 'Err'
                }
            ])
        } finally {
            setSubmitting(false);
        }
    }

    return (
        <main>
            <div>
                <div className="bg-slate-800">
                    <div className="p-8 text-center mt-4">
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
                                    {pingErr &&
                                        <><br/><span className="text-sm text-red-500"> ({pingErr})</span></>
                                    }
                                </div>
                            }
                            {pingMs >= 0 &&
                                <div>
                                    <span
                                        className="text-sm text-sky-700">Remote is up: connected in {pingMs} ms.</span>
                                </div>
                            }
                            <form className="flex items-center"
                                  onSubmit={async e => {
                                      e.preventDefault();
                                      await ping();
                                  }}>
                                <input
                                    className="w-72 h-10 m-4 mr-1 p-2 pl-4 pr-4 bg-neutral-800 rounded-xl text-zinc-200  placeholder-stone-600"
                                    type="text" placeholder="address:port"
                                    onChange={e => {
                                        setRemote(parseRemote(e.target.value))
                                    }}/>
                                <button
                                    className="cursor-pointer pl-2 pr-2 h-10 rounded-xl text-zinc-200
                            bg-teal-600 active:bg-teal-800 hover:bg-teal-700 transition select-none"
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
                                                return <li><span className="text-stone-300">{ln.message}</span></li>
                                            case "Err":
                                                return <li><span className="text-red-600">{ln.message}</span></li>
                                            default:
                                                return <li><span>?? {ln.message}</span></li>
                                        }
                                    })}
                                </ol>
                                <div
                                    className="rounded-b-xl bg-zinc-900 border-t-3 border-slate-950">
                                    <form ref={formRef} className="flex w-full justify-between" id="submit-form"
                                          onSubmit={submit} autoComplete="false">
                                        <input className="p-2 w-full placeholder-stone-600" placeholder="Command..."
                                               onChange={(e) => setPrompt(e.target.value)}/>
                                        <button
                                            className="p-2 pr-3 pl-3 bg-zinc-950 cursor-pointer active:bg-slate-950 hover:bg-stone-950 transition"
                                            disabled={submitting}
                                            type="submit">
                                            {submitting ? "..." : "Submit"}
                                        </button>
                                    </form>
                                </div>
                            </div>
                        </div>
                        <div className="m-auto mt-4 w-4/5">
                            <div className="flex justify-start space-x-2.5">
                                <div className="bg-red-500 rounded-lg p-2 pl-3 pr-3 font-semibold tracking-widest
                                active:bg-red-800 hover:bg-red-700 transition cursor-pointer select-none"
                                     onClick={() => reset()}>
                                    Reset
                                </div>
                                <input className="bg-slate-900 rounded-lg p-2 pl-3 pr-3"
                                       type="text" placeholder="Token..."
                                       onChange={(e) => setResetTarget(e.target.value)}/>
                            </div>
                        </div>
                    </div>
                </div>
                <div className="p-4 mt-2 border-t-2 border-slate-900 pb-16">
                    <p className="font-semibold text-sm text-center mr-auto ml-auto text-slate-500 author italic underline-offset-1">
                        Author: HerrPhoenix
                    </p>
                </div>
            </div>
        </main>
    );
}

export default App;
