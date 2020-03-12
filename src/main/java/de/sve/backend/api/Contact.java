package de.sve.backend.api;

import javax.ws.rs.POST;
import javax.ws.rs.Path;
import javax.ws.rs.Produces;
import javax.ws.rs.core.MediaType;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.ContactManager;
import de.sve.backend.model.contact.Email;
import de.sve.backend.model.contact.Message;

@Path("/contact")
@SuppressWarnings("static-method")
public class Contact {

	private static final Logger LOG = LoggerFactory.getLogger(Contact.class);

	@Path("/message")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void message(Message message) throws BackendException {
		try {
			ContactManager.message(message);
		} catch (Throwable t) {
			String msg = "Error while sending message"; //$NON-NLS-1$
			LOG.error(msg, t);
			throw new BackendException(msg, t);
		}
	}

	@Path("/email")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void email(Email email) throws BackendException {
		try {
			ContactManager.email(email);
		} catch (Throwable t) {
			String msg = "Error while sending email"; //$NON-NLS-1$
			LOG.error(msg, t);
			throw new BackendException(msg, t);
		}
	}

}
