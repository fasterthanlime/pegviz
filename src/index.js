document.addEventListener("click", (ev) => {
  if (ev.ctrlKey && ev.target.classList.contains("rule")) {
    ev.preventDefault();
    let text = ev.target.innerText;
    navigator.clipboard.writeText(text);

    let notifs = document.getElementById("notifications");
    let child = document.createElement("div");
    child.classList.add("notification");
    child.innerHTML = `Copied "${text}" to clipboard!`;
    notifs.appendChild(child);

    setTimeout(() => {
        child.classList.add("dead");
        setTimeout(() => {
            child.remove();
        }, 1000);
    }, 1000);
  }
});
