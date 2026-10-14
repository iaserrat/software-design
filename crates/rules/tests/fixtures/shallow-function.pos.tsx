function Title({ text }: { text: string }) {
  return <h1 className="title">{text}</h1>;
}
export function Page() { return <main><Title text="Home" /></main>; }
