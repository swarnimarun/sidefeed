import { LoginForm } from '@/components/login-form';
import { Button } from '@/components/ui/button';
import { ThemeToggle } from '@/components/theme-toggle';
import { Link, Outlet, createRootRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'

export const Route = createRootRoute({
  component: RootComponent,
})

function Nav() {
  return (
    <div className='flex justify-between'>
      <div className="p-2 flex gap-2 text-lg">
        <Button variant={"link"}>
          <Link
            to="/"
            activeProps={{
              className: 'underline',
            }}
            activeOptions={{ exact: true }}
          >
            Home
          </Link>
        </Button>{' '}
        <Button variant={"link"}>
          <Link
            to="/about"
            activeProps={{
              className: 'underline',
            }}
          >
            About
          </Link>
        </Button>
        <Button variant={"link"}>
          <Link
            to="/resume"
            activeProps={{
              className: 'underline',
            }}
          >
            Resume
          </Link>
        </Button>
      </div>

      <div className='m-2 flex gap-2'>
        <ThemeToggle/>
        <Button onClick={() => {
          console.log("logout")
          deleteCookie('token');
          location.reload();
        }}>Logout</Button>
      </div>
    </div>
  )
}

function getCookie(name: string) {
  const value = `; ${document.cookie}`;
  const parts = value.split(`; ${name}=`);
  if (parts.length === 2) return parts.pop()?.split(';').shift();
}

function deleteCookie(name: string) {
  document.cookie = name + "=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/;";
}


function RootComponent() {
  const [authToken, setAuthToken] = useState<string | null>(null);

  useEffect(() => {
    const cookie = getCookie('token');
    console.log("token = ", cookie, "authToken =", authToken);
    if (cookie && cookie !== authToken) 
      setAuthToken(cookie);
  });

  if (!authToken)
    return <div className="flex justify-center items-center h-screen">
        <LoginForm className='min-w-[490px] pb-70'/>
      </div>

  return (
    <>
      <div className="xl:max-w-5/6 2xl:max-w-3/4 3xl:max-w-2/3 mx-auto">
        <Nav/>
        <hr />
        <Outlet />
      </div>
    </>
  )
}
