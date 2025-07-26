// import { Button } from '@/components/ui/button'
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
  component: HomeComponent,
});

function HomeComponent() {
  // const clickMe = () => {
  //   alert("Button was Pressed!");
  // };
  return <div></div>;
}
