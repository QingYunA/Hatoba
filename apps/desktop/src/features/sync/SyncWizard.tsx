import { useState, type Dispatch, type SetStateAction } from "react";
import type { SyncConfigInput } from "@/ipc/types";
import { D1Step } from "./D1Step";
import { MethodStep } from "./MethodStep";
import { PasswordStep } from "./PasswordStep";
import { EMPTY_D1, EMPTY_WORKER, type D1Form, type Method, type WorkerForm } from "./wizardTypes";
import { WorkerStep } from "./WorkerStep";

/** Cloud Sync setup (design §05): 接入方式 → 连接 → 主密码. Mounted by the sync page while sync is off. */
export function SyncWizard() {
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [method, setMethod] = useState<Method>("worker");
  const [worker, setWorker] = useState<WorkerForm>(EMPTY_WORKER);
  const [d1, setD1] = useState<D1Form>(EMPTY_D1);
  const [config, setConfig] = useState<SyncConfigInput | null>(null);

  const patch = <F,>(set: Dispatch<SetStateAction<F>>) => (p: Partial<F>) => set((f) => ({ ...f, ...p }));

  if (step === 3 && config) return <PasswordStep config={config} onBack={() => setStep(2)} />;
  if (step === 2) {
    const next = (c: SyncConfigInput) => {
      setConfig(c);
      setStep(3);
    };
    return method === "worker" ? (
      <WorkerStep form={worker} update={patch(setWorker)} onBack={() => setStep(1)} onNext={next} />
    ) : (
      <D1Step form={d1} update={patch(setD1)} onBack={() => setStep(1)} onNext={next} />
    );
  }
  return <MethodStep method={method} onChange={setMethod} onNext={() => setStep(2)} />;
}
