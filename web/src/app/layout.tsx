import type { Metadata } from "next";
import { Archivo, IBM_Plex_Mono, Newsreader } from "next/font/google";
import "./globals.css";
import { Masthead } from "@/components/masthead";

const newsreader = Newsreader({
  subsets: ["latin"],
  variable: "--font-newsreader",
  display: "swap",
  style: ["normal", "italic"],
});

const archivo = Archivo({
  subsets: ["latin"],
  variable: "--font-archivo",
  display: "swap",
});

const plexMono = IBM_Plex_Mono({
  subsets: ["latin"],
  variable: "--font-plex-mono",
  display: "swap",
  weight: ["400", "500", "600"],
});

export const metadata: Metadata = {
  title: "Daymark — the morning edition for perishable retail",
  description:
    "A decision brief for grocery fresh sections: orders, markdowns and promotions, every figure traceable to a demand model running on AMD Instinct.",
  icons: { icon: "/icon.svg" },
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body
        className={`${newsreader.variable} ${archivo.variable} ${plexMono.variable} min-h-screen`}
      >
        <Masthead />
        <main className="mx-auto max-w-[1180px] px-4 pb-24 sm:px-6">{children}</main>
        <footer className="rule-top mt-12 py-6 text-center text-[11px] tracking-wide text-[var(--color-muted)] uppercase">
          Daymark · a working product on AMD Instinct · source data: Corporación Favorita
          (public benchmark)
        </footer>
      </body>
    </html>
  );
}
