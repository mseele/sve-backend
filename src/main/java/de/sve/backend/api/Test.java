package de.sve.backend.api;

import javax.ws.rs.GET;
import javax.ws.rs.Path;
import javax.ws.rs.Produces;
import javax.ws.rs.core.MediaType;

import de.sve.backend.mail.Mail;
import de.sve.backend.mail.MailAccount;

@Path("test")
public class Test {

	@GET
	@Produces(MediaType.TEXT_PLAIN)
	public String getMessage() {
		boolean result = Mail.via(MailAccount.FITNESS)
			.to("mseele@gmail.com")
			.bcc(MailAccount.FITNESS.email())
			.subject("Cool, funktioniert")
			.content("Ja, es tut :)").send();
		return "Email send: " + result; //$NON-NLS-1$
	}

}
