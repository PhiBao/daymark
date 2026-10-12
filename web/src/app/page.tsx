import BriefClient from "@/components/brief/brief-client";
import { BriefFacts, BriefResponse, Health, Meta, getJSON } from "@/lib/api";

export const instant = false;


export default async function Page(props: {
  searchParams: Promise<{ store?: string; date?: string }>;
}) {
  const sp = await props.searchParams;
  const store = Number(sp.store ?? "2");
  const date = sp.date ?? "2017-08-14";

  let facts: BriefFacts | null = null;
  let published: BriefResponse | null = null;
  let model = "";
  let stores: { store_nbr: number; city: string }[] = [];
  try {
    const [f, h, m] = await Promise.all([
      getJSON<{ facts: BriefFacts }>(`/api/brief/${store}?date=${date}`),
      getJSON<Health>(`/api/health`),
      getJSON<Meta>(`/api/meta`),
    ]);
    facts = f.facts;
    model = h.llm.reported_model ?? h.llm.model ?? "";
    stores = m.stores.map((s) => ({ store_nbr: s.store_nbr, city: s.city }));
  } catch {
    facts = null;
  }
  if (facts) {
    try {
      published = await getJSON<BriefResponse>(`/api/brief/${store}/published?date=${date}`);
    } catch {
      published = null; // not printed yet — the manager still sees the numbers
    }
  }

  return (
    <BriefClient
      store={store}
      date={date}
      initialFacts={facts}
      initialBrief={published}
      model={model}
      stores={stores}
    />
  );
}
