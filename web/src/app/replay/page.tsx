import ReplayClient from "@/components/replay/replay-client";

export const instant = false;


export default async function Page(props: { searchParams: Promise<{ store?: string }> }) {
  const sp = await props.searchParams;
  const store = Number(sp.store ?? "2");
  return <ReplayClient store={store} />;
}
