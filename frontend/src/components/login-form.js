import { jsx as _jsx, Fragment as _Fragment, jsxs as _jsxs } from "react/jsx-runtime";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle, } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useState } from "react";
import { useForm } from "@tanstack/react-form";
function FieldInfo({ field }) {
    return (_jsxs(_Fragment, { children: [field.state.meta.isTouched && !field.state.meta.isValid ? (_jsx("em", { children: field.state.meta.errors.join(", ") })) : null, field.state.meta.isValidating ? "Validating..." : null] }));
}
export function LoginForm({ className, ...props }) {
    const [formMode, setFormMode] = useState("login");
    if (formMode === "login") {
        console.log("login");
        const form = useForm({
            defaultValues: {
                email: "",
                password: "",
            },
            onSubmit: async ({ value }) => {
                // Do something with form data
                console.log(value);
            },
        });
        return (_jsx("div", { className: cn("flex flex-col gap-6", className), ...props, children: _jsxs(Card, { children: [_jsxs(CardHeader, { children: [_jsx(CardTitle, { children: "Login to your account" }), _jsx(CardDescription, { children: "Enter your email below to login to your account" })] }), _jsx(CardContent, { children: _jsxs("form", { onSubmit: (e) => {
                                e.preventDefault();
                                e.stopPropagation();
                                form.handleSubmit();
                            }, children: [_jsxs("div", { className: "flex flex-col gap-6", children: [_jsx("div", { className: "grid gap-3", children: _jsx(form.Field, { name: "email", validators: {
                                                    onChange: ({ value }) => !value
                                                        ? "Email is required"
                                                        : value.length < 7
                                                            ? "email needs to be atleast 7 characters"
                                                            : undefined,
                                                    onChangeAsyncDebounceMs: 500,
                                                    onChangeAsync: async ({ value }) => {
                                                        await new Promise((resolve) => setTimeout(resolve, 1000));
                                                        return (value.includes("error") &&
                                                            'No "error" allowed in first name');
                                                    },
                                                }, children: (field) => {
                                                    // Avoid hasty abstractions. Render props are great!
                                                    return (_jsxs(_Fragment, { children: [_jsx(Label, { htmlFor: field.name, children: "Email" }), _jsx(Input, { id: field.name, type: "email", placeholder: "m@example.com", name: field.name, required: true, value: field.state.value, onBlur: field.handleBlur, onChange: (e) => field.handleChange(e.target.value) }), _jsx(FieldInfo, { field: field })] }));
                                                } }) }), _jsx("div", { className: "grid gap-3", children: _jsx(form.Field, { name: "password", validators: {
                                                    onChange: ({ value }) => !value
                                                        ? "A password is required"
                                                        : value.length < 13
                                                            ? "Password must be at least 13 characters"
                                                            : undefined,
                                                    onChangeAsyncDebounceMs: 500,
                                                    onChangeAsync: async ({ value }) => {
                                                        await new Promise((resolve) => setTimeout(resolve, 1000));
                                                        return (value.includes("error") &&
                                                            'No "error" allowed in first name');
                                                    },
                                                }, children: (field) => {
                                                    return (_jsxs(_Fragment, { children: [_jsx("div", { className: "flex items-center", children: _jsx(Label, { htmlFor: field.name, children: "Password" }) }), _jsx(Input, { id: field.name, name: field.name, value: field.state.value, onBlur: field.handleBlur, onChange: (e) => field.handleChange(e.target.value), type: "password", required: true }), _jsx(FieldInfo, { field: field })] }));
                                                } }) }), _jsxs("div", { className: "flex flex-col gap-3", children: [_jsx(form.Subscribe, { selector: (state) => [state.canSubmit, state.isSubmitting], children: ([canSubmit, isSubmitting]) => (_jsx(Button, { type: "submit", className: "w-full", disabled: !canSubmit, children: isSubmitting ? "..." : "Login" })) }), _jsx(Button, { variant: "outline", className: "w-full", disabled: true, children: "Login with Google" })] })] }), _jsxs("div", { className: "mt-4 text-center text-sm", children: ["Don't have an account?", " ", _jsx("a", { href: "#", className: "underline underline-offset-4", onClick: () => {
                                                setFormMode("signup");
                                            }, children: "Sign up" })] })] }) })] }) }));
    }
    else {
        return (_jsx("div", { className: cn("flex flex-col gap-6", className), ...props, children: _jsxs(Card, { children: [_jsxs(CardHeader, { children: [_jsx(CardTitle, { children: "Sign up to a new account" }), _jsx(CardDescription, { children: "Enter your email and password" })] }), _jsx(CardContent, { children: _jsxs("form", { children: [_jsxs("div", { className: "flex flex-col gap-6", children: [_jsxs("div", { className: "grid gap-3", children: [_jsx(Label, { htmlFor: "email", children: "Email" }), _jsx(Input, { id: "email", type: "email", placeholder: "m@example.com", required: true })] }), _jsxs("div", { className: "grid gap-3", children: [_jsx("div", { className: "flex items-center", children: _jsx(Label, { htmlFor: "password", children: "Password" }) }), _jsx(Input, { id: "password", type: "password", required: true })] }), _jsx("div", { className: "flex flex-col gap-3", children: _jsx(Button, { type: "submit", className: "w-full", children: "Signup" }) })] }), _jsxs("div", { className: "mt-4 text-center text-sm", children: ["Already have an account?", " ", _jsx("a", { href: "#", className: "underline underline-offset-4", onClick: () => {
                                                setFormMode("login");
                                            }, children: "Login" })] })] }) })] }) }));
    }
}
