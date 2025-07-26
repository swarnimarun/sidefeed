import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useState } from "react";
import { useForm } from "@tanstack/react-form";
import type { AnyFieldApi } from "@tanstack/react-form";

function FieldInfo({ field }: { field: AnyFieldApi }) {
  return (
    <>
      {field.state.meta.isTouched && !field.state.meta.isValid ? (
        <em>{field.state.meta.errors.join(", ")}</em>
      ) : null}
      {field.state.meta.isValidating ? "Validating..." : null}
    </>
  );
}

export function LoginForm({
  className,
  ...props
}: React.ComponentProps<"div">) {
  const [formMode, setFormMode] = useState<"login" | "signup">("login");


  if (formMode === "login") {
    const { handleSubmit, Field, Subscribe } = useForm({
      defaultValues: {
        email: "",
        password: "",
      },
      onSubmit: async ({ value }) => {
        try {
          let response = await fetch('/api/auth/login', {
            method: "POST",
            headers: {
              'Accept': 'application/json',
              'Content-Type': "application/json"
            },
            body: JSON.stringify({
              'email': value.email,
              'password': value.password,
            }),
          });
          if (response.status === 200) {
            let body = await response.json();
            console.log(body);
            document.cookie = `token=${body.token}; SameSite=None; Secure`;
          }
        } catch (error) {
          console.error("failed to make auth login request: ", error);
        } finally {
          location.reload();
        }
      },
    });
    return (
      <div className={cn("flex flex-col gap-6", className)} {...props}>
        <Card>
          <CardHeader>
            <CardTitle>Login to your account</CardTitle>
            <CardDescription>
              Enter your email below to login to your account
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleSubmit();
              }}
            >
              <div className="flex flex-col gap-6">
                <div className="grid gap-3">
                  <Field
                    name="email"
                    validators={{
                      onChange: ({ value }) =>
                        !value
                          ? "Email is required"
                          : value.length < 7
                            ? "email needs to be atleast 7 characters"
                            : undefined,
                      onChangeAsyncDebounceMs: 500,
                      onChangeAsync: async ({ value }) => {
                        // await new Promise((resolve) =>
                        //   setTimeout(resolve, 1000),
                        // );
                        return (
                          value.includes("error") &&
                          'No "error" allowed in first name'
                        );
                      },
                    }}
                    children={(field) => {
                      // Avoid hasty abstractions. Render props are great!
                      return (
                        <>
                          <Label htmlFor={field.name}>Email</Label>
                          <Input
                            id={field.name}
                            type="email"
                            placeholder="m@example.com"
                            name={field.name}
                            required
                            value={field.state.value}
                            onBlur={field.handleBlur}
                            onChange={(e) => field.handleChange(e.target.value)}
                          />
                          <FieldInfo field={field} />
                        </>
                      );
                    }}
                  />
                </div>
                <div className="grid gap-3">
                  <Field
                    name="password"
                    validators={{
                      onChange: ({ value }) =>
                        !value
                          ? "A password is required"
                          : value.length < 7
                            ? "Password must be at least 7 characters"
                            : undefined,
                      onChangeAsyncDebounceMs: 500,
                      onChangeAsync: async ({ value }) => {
                        // await new Promise((resolve) =>
                        //   setTimeout(resolve, 1000),
                        // );
                        return (
                          value.includes("error") &&
                          'No "error" allowed in first name'
                        );
                      },
                    }}
                    children={(field) => {
                      return (
                        <>
                          <div className="flex items-center">
                            <Label htmlFor={field.name}>Password</Label>
                          </div>
                          <Input
                            id={field.name}
                            name={field.name}
                            value={field.state.value}
                            onBlur={field.handleBlur}
                            onChange={(e) => field.handleChange(e.target.value)}
                            type="password"
                            required
                          />
                          <FieldInfo field={field} />
                        </>
                      );
                    }}
                  />
                </div>
                <div className="flex flex-col gap-3">
                  <Subscribe
                    selector={(state) => [state.canSubmit, state.isSubmitting]}
                    children={([canSubmit, isSubmitting]) => (
                      <Button
                        type="submit"
                        className="w-full"
                        disabled={!canSubmit}
                      >
                        {isSubmitting ? "..." : "Login"}
                      </Button>
                    )}
                  />
                  <Button variant="outline" className="w-full" disabled={true}>
                    Login with Google
                  </Button>
                </div>
              </div>
              <div className="mt-4 text-center text-sm">
                Don&apos;t have an account?{" "}
                <a
                  href="#"
                  className="underline underline-offset-4"
                  onClick={() => {
                    setFormMode("signup");
                  }}
                >
                  Sign up
                </a>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    );
  } else {
    const { handleSubmit, Field, Subscribe } = useForm({
      defaultValues: {
        name: "",
        email: "",
        password: "",
      },
      onSubmit: async ({ value }) => {
        try {
          let response = await fetch('/api/auth/register', {
            method: "POST",
            headers: {
              'Accept': 'application/json',
              'Content-Type': "application/json"
            },
            body: JSON.stringify({
              'name': value.name,
              'email': value.email,
              'password': value.password,
            }),
          });
          if (response.status === 200) {
            let body = await response.json();
            console.log(body);
            document.cookie = `token=${body.token}; SameSite=None; Secure`;
          }
        } catch (error) {
          console.error("failed to make auth login request: ", error);
        } finally {
          location.reload();
        }
      },
    });
    return (
      <div className={cn("flex flex-col gap-6", className)} {...props}>
        <Card>
          <CardHeader>
            <CardTitle>Sign up to a new account</CardTitle>
            <CardDescription>Enter your email and password</CardDescription>
          </CardHeader>
          <CardContent>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleSubmit();
              }}
            >
              <div className="flex flex-col gap-6">
                <div className="grid gap-3">
                  <Field
                    name="name"
                    validators={{
                      onChange: ({ value }) =>
                        !value
                          ? "Email is required"
                          : value.length < 7
                            ? "email needs to be atleast 7 characters"
                            : undefined,
                      onChangeAsyncDebounceMs: 500,
                      onChangeAsync: async ({ value }) => {
                        // await new Promise((resolve) =>
                        //   setTimeout(resolve, 1000),
                        // );
                        return (
                          value.includes("error") &&
                          'No "error" allowed in first name'
                        );
                      },
                    }}
                    children={(field) => {
                      return (
                        <>
                          <Label htmlFor={field.name}>Name</Label>
                          <Input
                            id={field.name}
                            name={field.name}
                            type="text"
                            placeholder="Jon Dough"
                            required
                            value={field.state.value}
                            onBlur={field.handleBlur}
                            onChange={(e) => field.handleChange(e.target.value)}
                          />
                          <FieldInfo field={field} />
                        </>
                      );
                    }}
                  />
                </div>
                <div className="grid gap-3">
                  <Field
                    name="email"
                    validators={{
                      onChange: ({ value }) =>
                        !value
                          ? "Email is required"
                          : value.length < 7
                            ? "email needs to be atleast 7 characters"
                            : undefined,
                      onChangeAsyncDebounceMs: 500,
                      onChangeAsync: async ({ value }) => {
                        return (
                          value.includes("error") &&
                          'No "error" allowed in first name'
                        );
                      },
                    }}
                    children={(field) => {
                      return (
                        <>
                          <Label htmlFor={field.name}>Name</Label>
                          <Input
                            id={field.name}
                            name={field.name}
                            type="email"
                            placeholder="m@example.com"
                            required
                            value={field.state.value}
                            onBlur={field.handleBlur}
                            onChange={(e) => field.handleChange(e.target.value)}
                          />
                          <FieldInfo field={field} />
                        </>
                      );
                    }}
                  />
                </div>
                <div className="grid gap-3">
                  <Field
                    name="password"
                    validators={{
                      onChange: ({ value }) =>
                        !value
                          ? "Email is required"
                          : value.length < 7
                            ? "email needs to be atleast 7 characters"
                            : undefined,
                      onChangeAsyncDebounceMs: 500,
                      onChangeAsync: async ({ value }) => {
                        // await new Promise((resolve) =>
                        //   setTimeout(resolve, 1000),
                        // );
                        return (
                          value.includes("error") &&
                          'No "error" allowed in first name'
                        );
                      },
                    }}
                    children={(field) => {
                      return (
                        <>
                          <div className="flex items-center">
                            <Label htmlFor={field.name}>Password</Label>
                          </div>
                          <Input
                            id={field.name}
                            name={field.name}
                            value={field.state.value}
                            onBlur={field.handleBlur}
                            onChange={(e) => field.handleChange(e.target.value)}
                            type="password"
                            required
                          />
                          <FieldInfo field={field} />
                        </>
                      );
                    }}
                  />
                </div>
                <div className="flex flex-col gap-3">
                  <Subscribe
                    selector={(state) => [state.canSubmit, state.isSubmitting]}
                    children={([canSubmit, isSubmitting]) => (
                      <Button
                        type="submit"
                        className="w-full"
                        disabled={!canSubmit}
                      >
                        {isSubmitting ? "..." : "Signup"}
                      </Button>
                    )}
                  />
                </div>
              </div>
              <div className="mt-4 text-center text-sm">
                Already have an account?{" "}
                <a
                  href="#"
                  className="underline underline-offset-4"
                  onClick={() => {
                    setFormMode("login");
                  }}
                >
                  Login
                </a>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    );
  }
}
