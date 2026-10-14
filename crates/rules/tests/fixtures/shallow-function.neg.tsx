function Title({ text }: { text: string }) {
  return <h1 className="title">{text}</h1>;
}
export function Home() { return <Title text="Home" />; }
export function About() { return <Title text="About" />; }
