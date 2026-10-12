import BriefClient from "@/components/brief/brief-client";
import { BriefFacts, Health, getJSON } from "@/lib/api";

export const instant = false;


export default async function Page(props: {
  searchParams: Promise<{ store?: string; date?: string }>;
}) {
  const sp = await props.searchParams;
  const store = Number(sp.store ?? "2");
  const date = sp.date ?? "2017-08-14";

  let facts: BriefFacts | null = null;
  let model = "";
  try {
    const [f, h] = await Promise.all([
      getJSON<{ facts: BriefFacts }>(`/api/brief/${store}?date=${date}`),
      getJSON<Health>(`/api/health`),
    ]);
    facts = f.facts;
    model = h.llm.reported_model ?? h.llm.model ?? "";
  } catch {
    facts = null;
  }

  return <BriefClient store={store} date={date} initialFacts={facts} model={model} />;
}
